IMPLEMENTATION MODULE Http2ServerTestUtil;

  FROM SYSTEM IMPORT ADDRESS, ADR;
  FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear,
                       AppendByte, AppendChars, AppendView,
                       AsView, Len, GetByte;
  FROM Http2Types IMPORT Settings, FrameHeader, HeaderEntry,
                          FrameHeaderSize, PrefaceLen,
                          FrameData, FrameHeaders, FrameSettings,
                          FramePing, FrameGoaway, FrameRstStream,
                          FrameWindowUpdate, FrameContinuation,
                          FlagEndStream, FlagAck, FlagEndHeaders,
                          InitDefaultSettings;
  FROM Http2Frame IMPORT WritePreface, EncodeSettings, EncodeSettingsAck,
                          EncodePing, EncodeGoaway, EncodeRstStream,
                          EncodeWindowUpdate, EncodeDataHeader,
                          EncodeHeadersHeader, DecodeHeader;
  FROM Http2Hpack IMPORT DynTable, DynInit, EncodeHeaderBlock,
                          DecodeHeaderBlock;
  FROM Http2ServerConn IMPORT ConnPtr, ConnCreateTest, ConnFeedBytes,
                               ConnFlush, CpPreface, CpSettings, CpOpen;
  FROM Http2ServerTypes IMPORT Response;

  (* ── Frame builders ─────────────────────────────────── *)

  PROCEDURE BuildClientPreface(VAR buf: Buf);
  BEGIN
    WritePreface(buf);
  END BuildClientPreface;

  PROCEDURE BuildSettings(VAR buf: Buf; VAR s: Settings);
  BEGIN
    EncodeSettings(buf, s);
  END BuildSettings;

  PROCEDURE BuildSettingsAck(VAR buf: Buf);
  BEGIN
    EncodeSettingsAck(buf);
  END BuildSettingsAck;

  PROCEDURE BuildHeaders(VAR buf: Buf;
                         VAR dt: DynTable;
                         streamId: CARDINAL;
                         VAR headers: ARRAY OF HeaderEntry;
                         numHeaders: CARDINAL;
                         endStream: BOOLEAN);
  VAR
    hdrBuf: Buf;
  BEGIN
    Init(hdrBuf, 1024);
    EncodeHeaderBlock(hdrBuf, dt, headers, numHeaders);
    EncodeHeadersHeader(buf, streamId, hdrBuf.len, endStream, TRUE);
    AppendView(buf, AsView(hdrBuf));
    Free(hdrBuf);
  END BuildHeaders;

  PROCEDURE BuildData(VAR buf: Buf;
                      streamId: CARDINAL;
                      data: BytesView;
                      endStream: BOOLEAN);
  BEGIN
    EncodeDataHeader(buf, streamId, data.len, endStream);
    AppendView(buf, data);
  END BuildData;

  PROCEDURE BuildWindowUpdate(VAR buf: Buf;
                              streamId: CARDINAL;
                              increment: CARDINAL);
  BEGIN
    EncodeWindowUpdate(buf, streamId, increment);
  END BuildWindowUpdate;

  PROCEDURE BuildPing(VAR buf: Buf; data: BytesView);
  BEGIN
    EncodePing(buf, data, FALSE);
  END BuildPing;

  PROCEDURE BuildGoaway(VAR buf: Buf;
                        lastStreamId: CARDINAL;
                        errorCode: CARDINAL);
  BEGIN
    EncodeGoaway(buf, lastStreamId, errorCode);
  END BuildGoaway;

  PROCEDURE BuildRstStream(VAR buf: Buf;
                           streamId: CARDINAL;
                           errorCode: CARDINAL);
  BEGIN
    EncodeRstStream(buf, streamId, errorCode);
  END BuildRstStream;

  PROCEDURE BuildContinuation(VAR buf: Buf;
                              streamId: CARDINAL;
                              data: BytesView;
                              endHeaders: BOOLEAN);
  VAR
    hdr: FrameHeader;
    hdrBuf: Buf;
  BEGIN
    (* Manually encode CONTINUATION frame header *)
    hdr.length := data.len;
    hdr.ftype := FrameContinuation;
    IF endHeaders THEN
      hdr.flags := FlagEndHeaders;
    ELSE
      hdr.flags := 0;
    END;
    hdr.streamId := streamId;

    (* Encode 9-byte header *)
    AppendByte(buf, CHR((hdr.length DIV 65536) MOD 256));
    AppendByte(buf, CHR((hdr.length DIV 256) MOD 256));
    AppendByte(buf, CHR(hdr.length MOD 256));
    AppendByte(buf, CHR(hdr.ftype MOD 256));
    AppendByte(buf, CHR(hdr.flags MOD 256));
    AppendByte(buf, CHR((hdr.streamId DIV 16777216) MOD 128));
    AppendByte(buf, CHR((hdr.streamId DIV 65536) MOD 256));
    AppendByte(buf, CHR((hdr.streamId DIV 256) MOD 256));
    AppendByte(buf, CHR(hdr.streamId MOD 256));

    (* Append payload *)
    AppendView(buf, data);
  END BuildContinuation;

  (* ── Frame reader ───────────────────────────────────── *)

  PROCEDURE ReadNextFrame(VAR v: BytesView;
                          VAR hdr: FrameHeader;
                          VAR payload: BytesView): BOOLEAN;
  VAR
    ok: BOOLEAN;
    needed: CARDINAL;
    headerView: BytesView;
  BEGIN
    IF v.len < FrameHeaderSize THEN
      RETURN FALSE;
    END;

    (* Decode header from first 9 bytes *)
    headerView.base := v.base;
    headerView.len := FrameHeaderSize;
    DecodeHeader(headerView, hdr, ok);
    IF NOT ok THEN
      RETURN FALSE;
    END;

    needed := FrameHeaderSize + hdr.length;
    IF v.len < needed THEN
      RETURN FALSE;
    END;

    (* Set payload view *)
    payload.base := ADDRESS(LONGCARD(v.base) + LONGCARD(FrameHeaderSize));
    payload.len := hdr.length;

    (* Advance v past this frame *)
    v.base := ADDRESS(LONGCARD(v.base) + LONGCARD(needed));
    v.len := v.len - needed;

    RETURN TRUE;
  END ReadNextFrame;

  (* ── Test connection helpers ─────────────────────────── *)

  PROCEDURE FeedAndCollect(cp: ConnPtr;
                           VAR input: Buf;
                           VAR output: Buf);
  VAR
    v: BytesView;
  BEGIN
    (* Feed input bytes into the connection *)
    IF input.len > 0 THEN
      v := AsView(input);
      ConnFeedBytes(cp, v.base, v.len);
    END;

    (* Process the read buffer *)
    (* ConnOnEvent processes readBuf internally.
       For test connections, call it with fd=-1. *)
    ConnOnEvent(-1, 1, ADDRESS(cp));

    (* Collect output: copy writeBuf to output *)
    Clear(output);
    IF cp^.writeBuf.len > 0 THEN
      v := AsView(cp^.writeBuf);
      AppendView(output, v);
      Clear(cp^.writeBuf);
    END;
  END FeedAndCollect;

  PROCEDURE DoTestHandshake(cp: ConnPtr;
                            VAR output: Buf): BOOLEAN;
  VAR
    input: Buf;
    s: Settings;
    hdr: FrameHeader;
    payload: BytesView;
    outView: BytesView;
    ok, foundSettings, foundSettingsAck: BOOLEAN;
  BEGIN
    Init(input, 1024);

    (* Send client preface + default SETTINGS *)
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);

    (* Feed to connection and collect server response *)
    FeedAndCollect(cp, input, output);
    Free(input);

    (* Parse server output: should contain SETTINGS + SETTINGS ACK *)
    outView := AsView(output);
    foundSettings := FALSE;
    foundSettingsAck := FALSE;

    WHILE ReadNextFrame(outView, hdr, payload) DO
      IF hdr.ftype = FrameSettings THEN
        IF (hdr.flags = FlagAck) THEN
          foundSettingsAck := TRUE;
        ELSE
          foundSettings := TRUE;
          (* Now send SETTINGS ACK back *)
          Init(input, 64);
          BuildSettingsAck(input);
          FeedAndCollect(cp, input, output);
          Free(input);
        END;
      END;
    END;

    RETURN foundSettings;
  END DoTestHandshake;

  (* ── Convenience builders ───────────────────────────── *)

  PROCEDURE SetHeaderEntry(VAR e: HeaderEntry;
                           name: ARRAY OF CHAR; nameLen: CARDINAL;
                           value: ARRAY OF CHAR; valLen: CARDINAL);
  VAR
    i, lim: CARDINAL;
  BEGIN
    lim := nameLen;
    IF lim > 127 THEN lim := 127 END;
    FOR i := 0 TO lim - 1 DO
      e.name[i] := name[i];
    END;
    IF lim <= 127 THEN
      e.name[lim] := 0C;
    END;
    e.nameLen := lim;

    lim := valLen;
    IF lim > 4095 THEN lim := 4095 END;
    FOR i := 0 TO lim - 1 DO
      e.value[i] := value[i];
    END;
    IF lim <= 4095 THEN
      e.value[lim] := 0C;
    END;
    e.valLen := lim;
  END SetHeaderEntry;

  PROCEDURE StrLen(s: ARRAY OF CHAR): CARDINAL;
  VAR
    n: CARDINAL;
  BEGIN
    n := 0;
    WHILE (n <= HIGH(s)) AND (s[n] # 0C) DO
      INC(n);
    END;
    RETURN n;
  END StrLen;

  PROCEDURE BuildGET(VAR buf: Buf;
                     VAR dt: DynTable;
                     streamId: CARDINAL;
                     path: ARRAY OF CHAR);
  VAR
    headers: ARRAY [0..3] OF HeaderEntry;
    pathLen: CARDINAL;
  BEGIN
    SetHeaderEntry(headers[0], ":method", 7, "GET", 3);
    pathLen := StrLen(path);
    SetHeaderEntry(headers[1], ":path", 5, path, pathLen);
    SetHeaderEntry(headers[2], ":scheme", 7, "https", 5);
    SetHeaderEntry(headers[3], ":authority", 10, "localhost", 9);
    BuildHeaders(buf, dt, streamId, headers, 4, TRUE);
  END BuildGET;

  PROCEDURE BuildPOST(VAR buf: Buf;
                      VAR dt: DynTable;
                      streamId: CARDINAL;
                      path: ARRAY OF CHAR);
  VAR
    headers: ARRAY [0..3] OF HeaderEntry;
    pathLen: CARDINAL;
  BEGIN
    SetHeaderEntry(headers[0], ":method", 7, "POST", 4);
    pathLen := StrLen(path);
    SetHeaderEntry(headers[1], ":path", 5, path, pathLen);
    SetHeaderEntry(headers[2], ":scheme", 7, "https", 5);
    SetHeaderEntry(headers[3], ":authority", 10, "localhost", 9);
    BuildHeaders(buf, dt, streamId, headers, 4, FALSE);
  END BuildPOST;

END Http2ServerTestUtil.
