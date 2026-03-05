IMPLEMENTATION MODULE Http2ServerStream;

  FROM SYSTEM IMPORT ADDRESS;
  FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear,
                       AppendByte, AppendChars, AppendView,
                       AsView, GetByte;
  FROM Http2Types IMPORT HeaderEntry, MaxHeaders;
  FROM Http2Frame IMPORT EncodeHeadersHeader, EncodeDataHeader;
  FROM Http2Hpack IMPORT DynTable, EncodeHeaderBlock;
  FROM Http2Stream IMPORT H2Stream, InitStream, ConsumeSendWindow;
  FROM Http2ServerTypes IMPORT Request, Response, MaxStreamSlots,
                               MaxReqHeaders, MaxReqNameLen, MaxReqValueLen,
                               MaxMethodLen, MaxPathLen, MaxSchemeLen,
                               MaxAuthorityLen, MaxRespHeaders,
                               InitRequest, FreeRequest;

  (* ── String helpers ─────────────────────────────────────── *)

  (* Copy n characters from src to dst, truncating to dstHigh.
     Appends 0C terminator. *)
  PROCEDURE CopyChars(VAR src: ARRAY OF CHAR; srcLen: CARDINAL;
                      VAR dst: ARRAY OF CHAR; dstHigh: CARDINAL;
                      VAR dstLen: CARDINAL);
  VAR
    i, lim: CARDINAL;
  BEGIN
    lim := srcLen;
    IF lim > dstHigh THEN
      lim := dstHigh;
    END;
    FOR i := 0 TO lim - 1 DO
      dst[i] := src[i];
    END;
    IF lim <= dstHigh THEN
      dst[lim] := 0C;
    END;
    dstLen := lim;
  END CopyChars;

  (* Check if name[0..nameLen-1] equals literal string s.
     s must be 0C-terminated. *)
  PROCEDURE NameEquals(VAR name: ARRAY OF CHAR; nameLen: CARDINAL;
                       s: ARRAY OF CHAR): BOOLEAN;
  VAR
    i, sLen: CARDINAL;
  BEGIN
    sLen := 0;
    WHILE (sLen <= HIGH(s)) AND (s[sLen] # 0C) DO
      INC(sLen);
    END;
    IF nameLen # sLen THEN
      RETURN FALSE;
    END;
    FOR i := 0 TO nameLen - 1 DO
      IF name[i] # s[i] THEN
        RETURN FALSE;
      END;
    END;
    RETURN TRUE;
  END NameEquals;

  (* Convert a CARDINAL status code (0..999) to a string. *)
  PROCEDURE StatusToStr(code: CARDINAL; VAR buf: ARRAY OF CHAR);
  VAR
    d0, d1, d2: CARDINAL;
  BEGIN
    d2 := code MOD 10;
    code := code DIV 10;
    d1 := code MOD 10;
    d0 := code DIV 10;
    IF d0 > 9 THEN d0 := 9; END;
    buf[0] := CHR(ORD("0") + d0);
    buf[1] := CHR(ORD("0") + d1);
    buf[2] := CHR(ORD("0") + d2);
    IF HIGH(buf) >= 3 THEN
      buf[3] := 0C;
    END;
  END StatusToStr;

  (* Return the smaller of a and b. *)
  PROCEDURE MinCard(a, b: CARDINAL): CARDINAL;
  BEGIN
    IF a < b THEN RETURN a; ELSE RETURN b; END;
  END MinCard;

  (* Return the smaller of a (INTEGER) and b (CARDINAL) as CARDINAL.
     If a <= 0 returns 0. *)
  PROCEDURE MinIntCard(a: INTEGER; b: CARDINAL): CARDINAL;
  BEGIN
    IF a <= 0 THEN
      RETURN 0;
    END;
    IF CARDINAL(a) < b THEN
      RETURN CARDINAL(a);
    ELSE
      RETURN b;
    END;
  END MinIntCard;

  (* ── Slot lifecycle ─────────────────────────────────────── *)

  PROCEDURE SlotInit(VAR slot: StreamSlot);
  BEGIN
    slot.active := FALSE;
    slot.phase := PhIdle;
    slot.endRecvd := FALSE;
    slot.endSent := FALSE;
    FreeRequest(slot.req);
    InitRequest(slot.req);
  END SlotInit;

  PROCEDURE SlotFree(VAR slot: StreamSlot);
  BEGIN
    slot.active := FALSE;
    slot.phase := PhIdle;
    FreeRequest(slot.req);
    SlotInit(slot);
  END SlotFree;

  (* ── Header assembly ────────────────────────────────────── *)

  PROCEDURE AssembleHeaders(VAR slot: StreamSlot;
                            VAR decoded: ARRAY OF HeaderEntry;
                            numDecoded: CARDINAL;
                            endStream: BOOLEAN): BOOLEAN;
  VAR
    i: CARDINAL;
    dummy: CARDINAL;
    hasMethod, hasPath: BOOLEAN;
  BEGIN
    hasMethod := FALSE;
    hasPath := FALSE;

    FOR i := 0 TO numDecoded - 1 DO
      IF (decoded[i].nameLen > 0) AND (decoded[i].name[0] = ":") THEN
        (* Pseudo-header *)
        IF NameEquals(decoded[i].name, decoded[i].nameLen, ":method") THEN
          CopyChars(decoded[i].value, decoded[i].valLen,
                    slot.req.method, MaxMethodLen, dummy);
          hasMethod := TRUE;
        ELSIF NameEquals(decoded[i].name, decoded[i].nameLen, ":path") THEN
          CopyChars(decoded[i].value, decoded[i].valLen,
                    slot.req.path, MaxPathLen, dummy);
          hasPath := TRUE;
        ELSIF NameEquals(decoded[i].name, decoded[i].nameLen, ":scheme") THEN
          CopyChars(decoded[i].value, decoded[i].valLen,
                    slot.req.scheme, MaxSchemeLen, dummy);
        ELSIF NameEquals(decoded[i].name, decoded[i].nameLen, ":authority") THEN
          CopyChars(decoded[i].value, decoded[i].valLen,
                    slot.req.authority, MaxAuthorityLen, dummy);
        END;
      ELSE
        (* Regular header *)
        IF slot.req.numHeaders < MaxReqHeaders THEN
          CopyChars(decoded[i].name, decoded[i].nameLen,
                    slot.req.headers[slot.req.numHeaders].name,
                    MaxReqNameLen,
                    slot.req.headers[slot.req.numHeaders].nameLen);
          CopyChars(decoded[i].value, decoded[i].valLen,
                    slot.req.headers[slot.req.numHeaders].value,
                    MaxReqValueLen,
                    slot.req.headers[slot.req.numHeaders].valLen);
          INC(slot.req.numHeaders);
        END;
      END;
    END;

    (* Required pseudo-headers must be present *)
    IF (NOT hasMethod) OR (NOT hasPath) THEN
      RETURN FALSE;
    END;

    IF endStream THEN
      slot.endRecvd := TRUE;
      slot.phase := PhDispatched;
    ELSE
      slot.phase := PhData;
    END;

    RETURN TRUE;
  END AssembleHeaders;

  (* ── Data accumulation ──────────────────────────────────── *)

  PROCEDURE AccumulateData(VAR slot: StreamSlot;
                           data: BytesView;
                           endStream: BOOLEAN): BOOLEAN;
  BEGIN
    IF slot.phase # PhData THEN
      RETURN FALSE;
    END;

    AppendView(slot.req.body, data);
    slot.req.bodyLen := slot.req.body.len;

    IF endStream THEN
      slot.endRecvd := TRUE;
      slot.phase := PhDispatched;
    END;

    RETURN TRUE;
  END AccumulateData;

  (* ── Response sending ───────────────────────────────────── *)

  PROCEDURE SendResponse(VAR slot: StreamSlot;
                         VAR resp: Response;
                         VAR dynEnc: DynTable;
                         VAR outBuf: Buf;
                         maxFrameSize: CARDINAL;
                         VAR connWindow: INTEGER): CARDINAL;
  VAR
    hdrEntries: ARRAY [0..MaxRespHeaders] OF HeaderEntry;
    numEntries: CARDINAL;
    hdrBuf: Buf;
    startLen, totalAppended: CARDINAL;
    statusBuf: ARRAY [0..3] OF CHAR;
    i, j, copyLen: CARDINAL;
    bodyEnd: BOOLEAN;
    sendable, bodyOfs, remaining: CARDINAL;
    bodyView: BytesView;
    ok: BOOLEAN;
  BEGIN
    startLen := outBuf.len;
    totalAppended := 0;

    (* Build HPACK header entries: :status + response headers *)
    StatusToStr(resp.status, statusBuf);

    (* Entry 0: :status pseudo-header *)
    hdrEntries[0].name[0] := ":";
    hdrEntries[0].name[1] := "s";
    hdrEntries[0].name[2] := "t";
    hdrEntries[0].name[3] := "a";
    hdrEntries[0].name[4] := "t";
    hdrEntries[0].name[5] := "u";
    hdrEntries[0].name[6] := "s";
    hdrEntries[0].name[7] := 0C;
    hdrEntries[0].nameLen := 7;
    hdrEntries[0].value[0] := statusBuf[0];
    hdrEntries[0].value[1] := statusBuf[1];
    hdrEntries[0].value[2] := statusBuf[2];
    hdrEntries[0].value[3] := 0C;
    hdrEntries[0].valLen := 3;
    numEntries := 1;

    (* Copy response headers from ReqHeader format to HeaderEntry format *)
    FOR i := 0 TO resp.numHeaders - 1 DO
      IF numEntries <= MaxRespHeaders THEN
        (* Copy name *)
        copyLen := resp.headers[i].nameLen;
        IF copyLen > 127 THEN copyLen := 127; END;
        FOR j := 0 TO copyLen - 1 DO
          hdrEntries[numEntries].name[j] := resp.headers[i].name[j];
        END;
        IF copyLen <= 127 THEN
          hdrEntries[numEntries].name[copyLen] := 0C;
        END;
        hdrEntries[numEntries].nameLen := copyLen;

        (* Copy value *)
        copyLen := resp.headers[i].valLen;
        IF copyLen > 4095 THEN copyLen := 4095; END;
        FOR j := 0 TO copyLen - 1 DO
          hdrEntries[numEntries].value[j] := resp.headers[i].value[j];
        END;
        IF copyLen <= 4095 THEN
          hdrEntries[numEntries].value[copyLen] := 0C;
        END;
        hdrEntries[numEntries].valLen := copyLen;

        INC(numEntries);
      END;
    END;

    (* Encode header block into temporary buffer *)
    Init(hdrBuf, 1024);
    EncodeHeaderBlock(hdrBuf, dynEnc, hdrEntries, numEntries);

    (* Write HEADERS frame: header + encoded block *)
    bodyEnd := (resp.bodyLen = 0);
    EncodeHeadersHeader(outBuf, slot.stream.id, hdrBuf.len,
                        bodyEnd, TRUE);
    (* Append encoded header block bytes *)
    AppendView(outBuf, AsView(hdrBuf));
    Free(hdrBuf);

    slot.phase := PhResponding;

    (* Send DATA frames if there is a body *)
    IF resp.bodyLen > 0 THEN
      bodyOfs := 0;
      remaining := resp.bodyLen;

      WHILE remaining > 0 DO
        sendable := MinCard(remaining, maxFrameSize);
        sendable := MinIntCard(connWindow, sendable);

        IF sendable = 0 THEN
          (* Flow control exhausted; caller must call FlushData later *)
          totalAppended := outBuf.len - startLen;
          RETURN totalAppended;
        END;

        ok := ConsumeSendWindow(slot.stream, sendable);
        IF NOT ok THEN
          sendable := MinCard(sendable, CARDINAL(slot.stream.sendWindow));
          IF sendable = 0 THEN
            totalAppended := outBuf.len - startLen;
            RETURN totalAppended;
          END;
          ok := ConsumeSendWindow(slot.stream, sendable);
        END;
        connWindow := connWindow - INTEGER(sendable);

        remaining := remaining - sendable;
        bodyEnd := (remaining = 0);

        EncodeDataHeader(outBuf, slot.stream.id, sendable, bodyEnd);

        (* Append payload bytes from resp.body *)
        bodyView.base := resp.body.data;
        bodyView.len := resp.body.len;
        (* We need a sub-view from bodyOfs for sendable bytes *)
        bodyView.base := ADDRESS(LONGCARD(resp.body.data) + LONGCARD(bodyOfs));
        bodyView.len := sendable;
        AppendView(outBuf, bodyView);

        bodyOfs := bodyOfs + sendable;
      END;

      slot.endSent := TRUE;
      slot.phase := PhDone;
    ELSE
      slot.endSent := TRUE;
      slot.phase := PhDone;
    END;

    totalAppended := outBuf.len - startLen;
    RETURN totalAppended;
  END SendResponse;

  (* ── Flush remaining data ───────────────────────────────── *)

  PROCEDURE FlushData(VAR slot: StreamSlot;
                      VAR resp: Response;
                      VAR outBuf: Buf;
                      maxFrameSize: CARDINAL;
                      VAR connWindow: INTEGER): CARDINAL;
  VAR
    startLen, totalAppended: CARDINAL;
    bodyOfs, remaining, sendable: CARDINAL;
    bodyEnd: BOOLEAN;
    bodyView: BytesView;
    ok: BOOLEAN;
  BEGIN
    startLen := outBuf.len;

    IF slot.endSent THEN
      RETURN 0;
    END;

    (* Calculate how much has already been sent.
       resp.bodyLen is the total; the amount already sent is
       resp.bodyLen minus what remains.  We track "sent so far"
       by comparing resp.body.len (total body data) with bodyLen. *)
    (* We use the stream's sendWindow and connWindow to determine
       how much data was already acknowledged/sent.  The body offset
       is tracked by the difference between resp.bodyLen and remaining
       unsent data.  Since we don't store an explicit offset, we
       compute it from what's already been written. *)

    (* For simplicity, track remaining as resp.bodyLen minus already-
       consumed send window.  The caller should track bodyOfs externally
       or we derive it.  We'll use resp.body.len as total and assume
       all data up to resp.bodyLen - remaining was already sent.

       Since we don't have a separate offset field, we rely on the fact
       that ConsumeSendWindow tracks how much window has been used.
       The actual offset into the body is:
         bodyOfs = resp.bodyLen - (initial window - current window)
       But this is unreliable across WINDOW_UPDATEs.

       Simpler approach: use the body buf itself.  On each flush, send
       from the beginning, and truncate what's sent.  But that mutates
       the response body.

       Best approach: treat body as fully buffered, and use the slot's
       endSent flag to know if we're done.  Track offset via the
       difference between total bodyLen and stream sendWindow consumed. *)

    (* Practical approach: send from offset 0 since the HEADERS frame
       already told the peer about END_STREAM. The body data starts
       at resp.body.data.  We send whatever flow control allows now. *)
    bodyOfs := 0;
    remaining := resp.bodyLen;

    (* Skip past data already sent: if sendWindow was consumed,
       the sent amount = initialWindow - currentSendWindow.
       But we can also just walk the body from 0 and try to send
       all of it — ConsumeSendWindow will fail for already-consumed
       portions.  Actually that's wrong too.

       The safest approach: track sent offset in the body Buf by
       truncating sent data.  After sending N bytes, we shift the
       remaining data to the front.  But ByteBuf doesn't support
       that efficiently.

       Alternative: use resp.body.len as "remaining unsent".  When we
       send N bytes from the front, we Truncate to (len - N) after
       shifting.  But ByteBuf has no shift.

       Simplest: keep all body data, track offset by reducing bodyLen.
       After sending N bytes, set resp.bodyLen -= N.  The offset into
       resp.body.data is (resp.body.len - resp.bodyLen). *)
    bodyOfs := resp.body.len - resp.bodyLen;
    remaining := resp.bodyLen;

    WHILE remaining > 0 DO
      sendable := MinCard(remaining, maxFrameSize);
      sendable := MinIntCard(connWindow, sendable);

      IF sendable = 0 THEN
        totalAppended := outBuf.len - startLen;
        RETURN totalAppended;
      END;

      ok := ConsumeSendWindow(slot.stream, sendable);
      IF NOT ok THEN
        sendable := MinCard(sendable, CARDINAL(slot.stream.sendWindow));
        IF sendable = 0 THEN
          totalAppended := outBuf.len - startLen;
          RETURN totalAppended;
        END;
        ok := ConsumeSendWindow(slot.stream, sendable);
      END;
      connWindow := connWindow - INTEGER(sendable);

      remaining := remaining - sendable;
      resp.bodyLen := remaining;
      bodyEnd := (remaining = 0);

      EncodeDataHeader(outBuf, slot.stream.id, sendable, bodyEnd);

      bodyView.base := ADDRESS(LONGCARD(resp.body.data) + LONGCARD(bodyOfs));
      bodyView.len := sendable;
      AppendView(outBuf, bodyView);

      bodyOfs := bodyOfs + sendable;
    END;

    slot.endSent := TRUE;
    slot.phase := PhDone;

    totalAppended := outBuf.len - startLen;
    RETURN totalAppended;
  END FlushData;

  (* ── Slot pool management ───────────────────────────────── *)

  PROCEDURE AllocSlot(VAR slots: ARRAY OF StreamSlot;
                      streamId: CARDINAL;
                      initWindowSize: CARDINAL;
                      tablePtr: ADDRESS): CARDINAL;
  VAR
    i, limit: CARDINAL;
  BEGIN
    limit := HIGH(slots);
    IF limit >= MaxStreamSlots THEN
      limit := MaxStreamSlots - 1;
    END;
    FOR i := 0 TO limit DO
      IF NOT slots[i].active THEN
        SlotInit(slots[i]);
        slots[i].active := TRUE;
        InitStream(slots[i].stream, streamId, initWindowSize, tablePtr);
        RETURN i;
      END;
    END;
    RETURN MaxStreamSlots;
  END AllocSlot;

  PROCEDURE FindSlot(VAR slots: ARRAY OF StreamSlot;
                     streamId: CARDINAL): CARDINAL;
  VAR
    i, limit: CARDINAL;
  BEGIN
    limit := HIGH(slots);
    IF limit >= MaxStreamSlots THEN
      limit := MaxStreamSlots - 1;
    END;
    FOR i := 0 TO limit DO
      IF slots[i].active AND (slots[i].stream.id = streamId) THEN
        RETURN i;
      END;
    END;
    RETURN MaxStreamSlots;
  END FindSlot;

END Http2ServerStream.
