IMPLEMENTATION MODULE Http2Frame;

FROM ByteBuf IMPORT BytesView, Buf, AppendByte, AppendView,
                     ViewGetByte, AsView;
FROM Codec IMPORT Reader, Writer, InitReader, InitWriter,
                  ReadU8, ReadU32BE, WriteU8, WriteU32BE,
                  ReadSlice, Remaining;
FROM Http2Types IMPORT FrameHeader, Settings, FrameHeaderSize,
                       PrefaceLen, FrameSettings, FramePing,
                       FrameGoaway, FrameWindowUpdate,
                       FrameRstStream, FrameData, FrameHeaders,
                       FlagEndStream, FlagEndHeaders, FlagAck,
                       SetHeaderTableSize, SetEnablePush,
                       SetMaxConcurrentStreams, SetInitialWindowSize,
                       SetMaxFrameSize, SetMaxHeaderListSize,
                       ConnectionStreamId;

(* The 24-byte HTTP/2 connection preface string. *)
CONST
  P0  = "PRI * HTTP/2";  (* 12 chars *)
  P1  = ".0";            (* 2 chars *)

(* ── Helpers ───────────────────────────────────────────── *)

PROCEDURE WriteU24BE(VAR b: Buf; val: CARDINAL);
(* Encode a 24-bit value as 3 big-endian bytes. *)
BEGIN
  AppendByte(b, (val DIV 65536) MOD 256);
  AppendByte(b, (val DIV 256) MOD 256);
  AppendByte(b, val MOD 256)
END WriteU24BE;

PROCEDURE ReadU24BEfromView(v: BytesView; offset: CARDINAL): CARDINAL;
(* Read 3 bytes from a view as a 24-bit big-endian value. *)
VAR b0, b1, b2: CARDINAL;
BEGIN
  b0 := ViewGetByte(v, offset);
  b1 := ViewGetByte(v, offset + 1);
  b2 := ViewGetByte(v, offset + 2);
  RETURN b0 * 65536 + b1 * 256 + b2
END ReadU24BEfromView;

(* ── Frame header ──────────────────────────────────────── *)

PROCEDURE DecodeHeader(v: BytesView; VAR hdr: FrameHeader;
                       VAR ok: BOOLEAN);
VAR r: Reader;
    b0, b1, b2: CARDINAL;
    raw32: CARDINAL;
BEGIN
  ok := TRUE;
  IF v.len < FrameHeaderSize THEN
    ok := FALSE;
    RETURN
  END;
  InitReader(r, v);
  (* 24-bit length: 3 x ReadU8 *)
  b0 := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  b1 := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  b2 := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  hdr.length := b0 * 65536 + b1 * 256 + b2;
  hdr.ftype := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  hdr.flags := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  raw32 := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
  (* Mask off the reserved bit (bit 31) *)
  hdr.streamId := raw32 MOD 2147483648
END DecodeHeader;

PROCEDURE EncodeHeader(VAR b: Buf; hdr: FrameHeader);
BEGIN
  WriteU24BE(b, hdr.length);
  AppendByte(b, hdr.ftype MOD 256);
  AppendByte(b, hdr.flags MOD 256);
  (* Stream ID: ensure reserved bit is 0 *)
  AppendByte(b, (hdr.streamId DIV 16777216) MOD 128);
  AppendByte(b, (hdr.streamId DIV 65536) MOD 256);
  AppendByte(b, (hdr.streamId DIV 256) MOD 256);
  AppendByte(b, hdr.streamId MOD 256)
END EncodeHeader;

(* ── Connection preface ────────────────────────────────── *)

PROCEDURE WritePreface(VAR b: Buf);
(* PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n *)
BEGIN
  (* "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" = 24 bytes *)
  AppendByte(b, ORD("P"));  AppendByte(b, ORD("R"));
  AppendByte(b, ORD("I"));  AppendByte(b, ORD(" "));
  AppendByte(b, ORD("*"));  AppendByte(b, ORD(" "));
  AppendByte(b, ORD("H"));  AppendByte(b, ORD("T"));
  AppendByte(b, ORD("T"));  AppendByte(b, ORD("P"));
  AppendByte(b, ORD("/"));  AppendByte(b, ORD("2"));
  AppendByte(b, ORD("."));  AppendByte(b, ORD("0"));
  AppendByte(b, 13);        AppendByte(b, 10);  (* \r\n *)
  AppendByte(b, 13);        AppendByte(b, 10);  (* \r\n *)
  AppendByte(b, ORD("S"));  AppendByte(b, ORD("M"));
  AppendByte(b, 13);        AppendByte(b, 10);  (* \r\n *)
  AppendByte(b, 13);        AppendByte(b, 10)   (* \r\n *)
END WritePreface;

PROCEDURE CheckPreface(v: BytesView): BOOLEAN;
VAR expected: ARRAY [0..23] OF CARDINAL;
    i: CARDINAL;
BEGIN
  IF v.len < PrefaceLen THEN RETURN FALSE END;
  expected[0]  := ORD("P");  expected[1]  := ORD("R");
  expected[2]  := ORD("I");  expected[3]  := ORD(" ");
  expected[4]  := ORD("*");  expected[5]  := ORD(" ");
  expected[6]  := ORD("H");  expected[7]  := ORD("T");
  expected[8]  := ORD("T");  expected[9]  := ORD("P");
  expected[10] := ORD("/");  expected[11] := ORD("2");
  expected[12] := ORD(".");  expected[13] := ORD("0");
  expected[14] := 13;        expected[15] := 10;
  expected[16] := 13;        expected[17] := 10;
  expected[18] := ORD("S");  expected[19] := ORD("M");
  expected[20] := 13;        expected[21] := 10;
  expected[22] := 13;        expected[23] := 10;
  i := 0;
  WHILE i < PrefaceLen DO
    IF ViewGetByte(v, i) # expected[i] THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END CheckPreface;

(* ── SETTINGS ──────────────────────────────────────────── *)

PROCEDURE DecodeSettings(payload: BytesView;
                         VAR s: Settings; VAR ok: BOOLEAN);
VAR r: Reader;
    id, val: CARDINAL;
    idHi, idLo: CARDINAL;
BEGIN
  ok := TRUE;
  IF (payload.len MOD 6) # 0 THEN
    ok := FALSE;
    RETURN
  END;
  InitReader(r, payload);
  WHILE Remaining(r) >= 6 DO
    (* Setting ID is 16-bit big-endian *)
    idHi := ReadU8(r, ok); IF NOT ok THEN RETURN END;
    idLo := ReadU8(r, ok); IF NOT ok THEN RETURN END;
    id := idHi * 256 + idLo;
    val := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
    IF id = SetHeaderTableSize THEN
      s.headerTableSize := val
    ELSIF id = SetEnablePush THEN
      s.enablePush := val
    ELSIF id = SetMaxConcurrentStreams THEN
      s.maxConcurrentStreams := val
    ELSIF id = SetInitialWindowSize THEN
      s.initialWindowSize := val
    ELSIF id = SetMaxFrameSize THEN
      s.maxFrameSize := val
    ELSIF id = SetMaxHeaderListSize THEN
      s.maxHeaderListSize := val
    END
    (* Unknown settings are ignored per RFC *)
  END
END DecodeSettings;

PROCEDURE EncodeSettings(VAR b: Buf; s: Settings);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := 36;  (* 6 settings * 6 bytes each *)
  hdr.ftype := FrameSettings;
  hdr.flags := 0;
  hdr.streamId := ConnectionStreamId;
  EncodeHeader(b, hdr);
  (* Each setting: 2-byte ID (big-endian) + 4-byte value (big-endian) *)
  AppendByte(b, 0); AppendByte(b, SetHeaderTableSize);
  AppendByte(b, (s.headerTableSize DIV 16777216) MOD 256);
  AppendByte(b, (s.headerTableSize DIV 65536) MOD 256);
  AppendByte(b, (s.headerTableSize DIV 256) MOD 256);
  AppendByte(b, s.headerTableSize MOD 256);

  AppendByte(b, 0); AppendByte(b, SetEnablePush);
  AppendByte(b, (s.enablePush DIV 16777216) MOD 256);
  AppendByte(b, (s.enablePush DIV 65536) MOD 256);
  AppendByte(b, (s.enablePush DIV 256) MOD 256);
  AppendByte(b, s.enablePush MOD 256);

  AppendByte(b, 0); AppendByte(b, SetMaxConcurrentStreams);
  AppendByte(b, (s.maxConcurrentStreams DIV 16777216) MOD 256);
  AppendByte(b, (s.maxConcurrentStreams DIV 65536) MOD 256);
  AppendByte(b, (s.maxConcurrentStreams DIV 256) MOD 256);
  AppendByte(b, s.maxConcurrentStreams MOD 256);

  AppendByte(b, 0); AppendByte(b, SetInitialWindowSize);
  AppendByte(b, (s.initialWindowSize DIV 16777216) MOD 256);
  AppendByte(b, (s.initialWindowSize DIV 65536) MOD 256);
  AppendByte(b, (s.initialWindowSize DIV 256) MOD 256);
  AppendByte(b, s.initialWindowSize MOD 256);

  AppendByte(b, 0); AppendByte(b, SetMaxFrameSize);
  AppendByte(b, (s.maxFrameSize DIV 16777216) MOD 256);
  AppendByte(b, (s.maxFrameSize DIV 65536) MOD 256);
  AppendByte(b, (s.maxFrameSize DIV 256) MOD 256);
  AppendByte(b, s.maxFrameSize MOD 256);

  AppendByte(b, 0); AppendByte(b, SetMaxHeaderListSize);
  AppendByte(b, (s.maxHeaderListSize DIV 16777216) MOD 256);
  AppendByte(b, (s.maxHeaderListSize DIV 65536) MOD 256);
  AppendByte(b, (s.maxHeaderListSize DIV 256) MOD 256);
  AppendByte(b, s.maxHeaderListSize MOD 256)
END EncodeSettings;

PROCEDURE EncodeSettingsAck(VAR b: Buf);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := 0;
  hdr.ftype := FrameSettings;
  hdr.flags := FlagAck;
  hdr.streamId := ConnectionStreamId;
  EncodeHeader(b, hdr)
END EncodeSettingsAck;

(* ── PING ──────────────────────────────────────────────── *)

PROCEDURE EncodePing(VAR b: Buf; data: BytesView; isAck: BOOLEAN);
VAR hdr: FrameHeader;
    i: CARDINAL;
BEGIN
  hdr.length := 8;
  hdr.ftype := FramePing;
  IF isAck THEN hdr.flags := FlagAck ELSE hdr.flags := 0 END;
  hdr.streamId := ConnectionStreamId;
  EncodeHeader(b, hdr);
  (* Write exactly 8 bytes of opaque data *)
  i := 0;
  WHILE i < 8 DO
    IF i < data.len THEN
      AppendByte(b, ViewGetByte(data, i))
    ELSE
      AppendByte(b, 0)
    END;
    INC(i)
  END
END EncodePing;

(* ── GOAWAY ────────────────────────────────────────────── *)

PROCEDURE EncodeGoaway(VAR b: Buf; lastStreamId: CARDINAL;
                       errorCode: CARDINAL);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := 8;
  hdr.ftype := FrameGoaway;
  hdr.flags := 0;
  hdr.streamId := ConnectionStreamId;
  EncodeHeader(b, hdr);
  (* Last-Stream-ID: 4 bytes big-endian, reserved bit = 0 *)
  AppendByte(b, (lastStreamId DIV 16777216) MOD 128);
  AppendByte(b, (lastStreamId DIV 65536) MOD 256);
  AppendByte(b, (lastStreamId DIV 256) MOD 256);
  AppendByte(b, lastStreamId MOD 256);
  (* Error code: 4 bytes big-endian *)
  AppendByte(b, (errorCode DIV 16777216) MOD 256);
  AppendByte(b, (errorCode DIV 65536) MOD 256);
  AppendByte(b, (errorCode DIV 256) MOD 256);
  AppendByte(b, errorCode MOD 256)
END EncodeGoaway;

PROCEDURE DecodeGoaway(payload: BytesView;
                       VAR lastStreamId: CARDINAL;
                       VAR errorCode: CARDINAL;
                       VAR ok: BOOLEAN);
VAR r: Reader;
    raw: CARDINAL;
BEGIN
  ok := TRUE;
  IF payload.len < 8 THEN ok := FALSE; RETURN END;
  InitReader(r, payload);
  raw := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
  lastStreamId := raw MOD 2147483648;
  errorCode := ReadU32BE(r, ok)
END DecodeGoaway;

(* ── WINDOW_UPDATE ─────────────────────────────────────── *)

PROCEDURE EncodeWindowUpdate(VAR b: Buf; streamId: CARDINAL;
                             increment: CARDINAL);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := 4;
  hdr.ftype := FrameWindowUpdate;
  hdr.flags := 0;
  hdr.streamId := streamId;
  EncodeHeader(b, hdr);
  (* Window size increment: 4 bytes, reserved bit = 0 *)
  AppendByte(b, (increment DIV 16777216) MOD 128);
  AppendByte(b, (increment DIV 65536) MOD 256);
  AppendByte(b, (increment DIV 256) MOD 256);
  AppendByte(b, increment MOD 256)
END EncodeWindowUpdate;

PROCEDURE DecodeWindowUpdate(payload: BytesView;
                             VAR increment: CARDINAL;
                             VAR ok: BOOLEAN);
VAR r: Reader;
    raw: CARDINAL;
BEGIN
  ok := TRUE;
  IF payload.len < 4 THEN ok := FALSE; RETURN END;
  InitReader(r, payload);
  raw := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
  increment := raw MOD 2147483648
END DecodeWindowUpdate;

(* ── RST_STREAM ────────────────────────────────────────── *)

PROCEDURE EncodeRstStream(VAR b: Buf; streamId: CARDINAL;
                          errorCode: CARDINAL);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := 4;
  hdr.ftype := FrameRstStream;
  hdr.flags := 0;
  hdr.streamId := streamId;
  EncodeHeader(b, hdr);
  AppendByte(b, (errorCode DIV 16777216) MOD 256);
  AppendByte(b, (errorCode DIV 65536) MOD 256);
  AppendByte(b, (errorCode DIV 256) MOD 256);
  AppendByte(b, errorCode MOD 256)
END EncodeRstStream;

PROCEDURE DecodeRstStream(payload: BytesView;
                          VAR errorCode: CARDINAL;
                          VAR ok: BOOLEAN);
VAR r: Reader;
BEGIN
  ok := TRUE;
  IF payload.len < 4 THEN ok := FALSE; RETURN END;
  InitReader(r, payload);
  errorCode := ReadU32BE(r, ok)
END DecodeRstStream;

(* ── DATA ──────────────────────────────────────────────── *)

PROCEDURE EncodeDataHeader(VAR b: Buf; streamId: CARDINAL;
                           payloadLen: CARDINAL; endStream: BOOLEAN);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := payloadLen;
  hdr.ftype := FrameData;
  IF endStream THEN hdr.flags := FlagEndStream ELSE hdr.flags := 0 END;
  hdr.streamId := streamId;
  EncodeHeader(b, hdr)
END EncodeDataHeader;

(* ── HEADERS ───────────────────────────────────────────── *)

PROCEDURE EncodeHeadersHeader(VAR b: Buf; streamId: CARDINAL;
                              payloadLen: CARDINAL;
                              endStream: BOOLEAN;
                              endHeaders: BOOLEAN);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := payloadLen;
  hdr.ftype := FrameHeaders;
  hdr.flags := 0;
  IF endStream THEN hdr.flags := hdr.flags + FlagEndStream END;
  IF endHeaders THEN hdr.flags := hdr.flags + FlagEndHeaders END;
  hdr.streamId := streamId;
  EncodeHeader(b, hdr)
END EncodeHeadersHeader;

END Http2Frame.
