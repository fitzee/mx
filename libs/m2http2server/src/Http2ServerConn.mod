IMPLEMENTATION MODULE Http2ServerConn;

  FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
  FROM Storage IMPORT ALLOCATE, DEALLOCATE;
  FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear,
                       AppendByte, AppendChars, AppendView,
                       AsView, Len, GetByte;
  FROM Http2Types IMPORT FrameHeader, Settings, HeaderEntry,
                          MaxHeaders, FrameHeaderSize,
                          FrameData, FrameHeaders, FrameSettings,
                          FramePing, FrameGoaway, FrameWindowUpdate,
                          FrameRstStream, FramePriority,
                          FrameContinuation,
                          FlagEndStream, FlagAck, FlagEndHeaders,
                          PrefaceLen,
                          ErrNoError, ErrProtocol, ErrFlowControl,
                          ErrFrameSize, ErrStreamClosed,
                          DefaultWindowSize, DefaultInitialWindowSize,
                          DefaultMaxFrameSize,
                          InitDefaultSettings;
  FROM Http2Frame IMPORT DecodeHeader, EncodeHeader,
                          CheckPreface,
                          DecodeSettings, EncodeSettings, EncodeSettingsAck,
                          EncodePing, EncodeGoaway,
                          EncodeWindowUpdate, EncodeRstStream,
                          DecodeWindowUpdate, DecodeRstStream,
                          DecodeGoaway;
  FROM Http2Hpack IMPORT DynTable, DynInit, DecodeHeaderBlock;
  FROM Http2Stream IMPORT StreamTransTable, InitStreamTable,
                           H2Stream, InitStream, StreamStep,
                           ConsumeSendWindow, UpdateSendWindow,
                           ConsumeRecvWindow, UpdateRecvWindow,
                           StreamState, IsClosed;
  FROM Http2ServerTypes IMPORT MaxStreamSlots, MaxConns,
                                Request, Response, Status,
                                ConnBufSize, HandlerProc,
                                InitRequest, InitResponse,
                                FreeRequest, FreeResponse;
  FROM Http2ServerStream IMPORT StreamSlot, SlotInit, SlotFree,
                                 AssembleHeaders, AccumulateData,
                                 SendResponse, FlushData,
                                 AllocSlot, FindSlot,
                                 PhIdle, PhHeaders, PhData,
                                 PhDispatched, PhResponding, PhDone,
                                 PhHeld, PhDeferred;
  FROM Arena IMPORT Arena,
                     Init AS ArenaInit, Alloc AS ArenaAlloc,
                     Mark AS ArenaMark, ResetTo AS ArenaResetTo,
                     Clear AS ArenaClear;
  FROM Sockets IMPORT SockAddr;
  IMPORT TLS;
  IMPORT EventLoop;
  FROM Poller IMPORT EvRead, EvWrite, EvHup;

  (* ── Forward-declared server record access ──────────── *)

  (* We access the server record via the back-pointer.
     The server record layout is defined in Http2Server.mod.
     We access fields by known offsets through helper procedures
     that the server module provides.  For simplicity here, we
     define a minimal overlay type for the fields we need. *)

  TYPE
    (* Minimal overlay of fields we need from ServerRec.
       Must match Http2Server.mod layout exactly.  We only
       access: router, middleware, metrics, logger, eventLoop,
       serverSettings, serverOpts. *)
    ServerPtr = POINTER TO ServerOverlay;
    ServerOverlay = RECORD
      dummy: ADDRESS;  (* placeholder — actual access is through
                          exported procedures from Http2Server *)
    END;

  (* ── Internal helpers ───────────────────────────────── *)

  PROCEDURE MinCard(a, b: CARDINAL): CARDINAL;
  BEGIN
    IF a < b THEN RETURN a ELSE RETURN b END;
  END MinCard;

  (* Copy SockAddr to string representation *)
  PROCEDURE AddrToStr(VAR peer: SockAddr; VAR out: ARRAY OF CHAR);
  VAR
    i: CARDINAL;
    b: CARDINAL;
  BEGIN
    (* Simple dotted-quad output *)
    i := 0;
    b := ORD(peer.addrV4[0]) MOD 256;
    IF b >= 100 THEN
      out[i] := CHR(ORD("0") + b DIV 100); INC(i);
    END;
    IF b >= 10 THEN
      out[i] := CHR(ORD("0") + (b DIV 10) MOD 10); INC(i);
    END;
    out[i] := CHR(ORD("0") + b MOD 10); INC(i);
    out[i] := "."; INC(i);

    b := ORD(peer.addrV4[1]) MOD 256;
    IF b >= 100 THEN
      out[i] := CHR(ORD("0") + b DIV 100); INC(i);
    END;
    IF b >= 10 THEN
      out[i] := CHR(ORD("0") + (b DIV 10) MOD 10); INC(i);
    END;
    out[i] := CHR(ORD("0") + b MOD 10); INC(i);
    out[i] := "."; INC(i);

    b := ORD(peer.addrV4[2]) MOD 256;
    IF b >= 100 THEN
      out[i] := CHR(ORD("0") + b DIV 100); INC(i);
    END;
    IF b >= 10 THEN
      out[i] := CHR(ORD("0") + (b DIV 10) MOD 10); INC(i);
    END;
    out[i] := CHR(ORD("0") + b MOD 10); INC(i);
    out[i] := "."; INC(i);

    b := ORD(peer.addrV4[3]) MOD 256;
    IF b >= 100 THEN
      out[i] := CHR(ORD("0") + b DIV 100); INC(i);
    END;
    IF b >= 10 THEN
      out[i] := CHR(ORD("0") + (b DIV 10) MOD 10); INC(i);
    END;
    out[i] := CHR(ORD("0") + b MOD 10); INC(i);

    IF i <= HIGH(out) THEN
      out[i] := 0C;
    END;
  END AddrToStr;

  (* ── Connection creation ────────────────────────────── *)

  PROCEDURE ConnCreate(serverPtr: ADDRESS;
                       connId: CARDINAL;
                       clientFd: INTEGER;
                       peer: SockAddr;
                       VAR cp: ConnPtr): BOOLEAN;
  VAR
    i: CARDINAL;
    ok: BOOLEAN;
  BEGIN
    ALLOCATE(cp, TSIZE(ConnRec));
    IF cp = NIL THEN
      RETURN FALSE;
    END;

    cp^.id := connId;
    cp^.server := serverPtr;
    cp^.fd := clientFd;
    cp^.phase := CpPreface;

    cp^.tlsSess := NIL;
    cp^.tlsCtx := NIL;
    cp^.strm := NIL;

    (* H2 protocol state *)
    InitDefaultSettings(cp^.localSettings);
    InitDefaultSettings(cp^.remoteSettings);
    (* Server should not push *)
    cp^.localSettings.enablePush := 0;
    cp^.localSettings.maxConcurrentStreams := MaxStreamSlots;

    cp^.connSendWindow := DefaultWindowSize;
    cp^.connRecvWindow := DefaultWindowSize;
    cp^.lastPeerStream := 0;
    cp^.goawaySent := FALSE;
    cp^.goawayRecvd := FALSE;
    cp^.prefaceRecvd := FALSE;
    cp^.settingsAckRecvd := FALSE;
    cp^.numActive := 0;

    (* HPACK tables *)
    DynInit(cp^.dynEnc, 4096);
    DynInit(cp^.dynDec, 4096);

    (* Stream FSM table *)
    InitStreamTable(cp^.streamTable);

    (* Stream slots — null body pointers before SlotInit to prevent
       FreeRequest from freeing garbage in freshly ALLOCATE'd memory *)
    FOR i := 0 TO MaxStreamSlots - 1 DO
      cp^.slots[i].req.body.data := NIL;
      cp^.slots[i].req.body.len := 0;
      cp^.slots[i].req.body.cap := 0;
      SlotInit(cp^.slots[i]);
    END;

    (* I/O buffers *)
    Init(cp^.readBuf, ConnBufSize);
    Init(cp^.writeBuf, ConnBufSize);
    cp^.writeOff := 0;

    (* Frame parse state *)
    cp^.haveHeader := FALSE;

    (* Arena *)
    ALLOCATE(cp^.arenaBase, ArenaSize);
    IF cp^.arenaBase = NIL THEN
      Free(cp^.readBuf);
      Free(cp^.writeBuf);
      DEALLOCATE(cp, TSIZE(ConnRec));
      cp := NIL;
      RETURN FALSE;
    END;
    ArenaInit(cp^.arena, cp^.arenaBase, ArenaSize);

    (* Timers *)
    cp^.idleTimerId := -1;
    cp^.hsTimerId := -1;

    (* Remote address *)
    AddrToStr(peer, cp^.remoteAddr);
    cp^.startTick := 0;
    cp^.lastActive := 0;

    cp^.watching := FALSE;
    cp^.loop := NIL;
    cp^.deferredCount := 0;

    RETURN TRUE;
  END ConnCreate;

  (* ── Test connection (no TLS/Stream) ────────────────── *)

  PROCEDURE ConnCreateTest(serverPtr: ADDRESS;
                           connId: CARDINAL;
                           VAR cp: ConnPtr): BOOLEAN;
  VAR
    peer: SockAddr;
    i: CARDINAL;
  BEGIN
    peer.family := 2;
    peer.port := 0;
    peer.addrV4[0] := CHR(127);
    peer.addrV4[1] := CHR(0);
    peer.addrV4[2] := CHR(0);
    peer.addrV4[3] := CHR(1);

    IF NOT ConnCreate(serverPtr, connId, -1, peer, cp) THEN
      RETURN FALSE;
    END;

    (* Test connections skip TLS — go straight to preface wait *)
    cp^.phase := CpPreface;
    RETURN TRUE;
  END ConnCreateTest;

  (* ── Feed bytes for testing ─────────────────────────── *)

  PROCEDURE ConnFeedBytes(cp: ConnPtr;
                          data: ADDRESS;
                          len: CARDINAL);
  VAR
    v: BytesView;
  BEGIN
    v.base := data;
    v.len := len;
    AppendView(cp^.readBuf, v);
  END ConnFeedBytes;

  (* ── Frame processing ──────────────────────────────── *)

  PROCEDURE SendGoaway(cp: ConnPtr; errorCode: CARDINAL);
  BEGIN
    EncodeGoaway(cp^.writeBuf, cp^.lastPeerStream, errorCode);
    cp^.goawaySent := TRUE;
    cp^.phase := CpGoaway;
  END SendGoaway;

  PROCEDURE SendRstStream(cp: ConnPtr; streamId, errorCode: CARDINAL);
  BEGIN
    EncodeRstStream(cp^.writeBuf, streamId, errorCode);
  END SendRstStream;

  PROCEDURE SendWindowUpdate(cp: ConnPtr; streamId, increment: CARDINAL);
  BEGIN
    EncodeWindowUpdate(cp^.writeBuf, streamId, increment);
  END SendWindowUpdate;

  (* Process a complete frame given header + payload view *)
  PROCEDURE ProcessFrame(cp: ConnPtr;
                         VAR hdr: FrameHeader;
                         payload: BytesView);
  VAR
    ok: BOOLEAN;
    s: Settings;
    increment, errorCode, lastStream: CARDINAL;
    slotIdx: CARDINAL;
    decoded: ARRAY [0..15] OF HeaderEntry;
    numDecoded: CARDINAL;
    endStream, endHeaders: BOOLEAN;
    resp: Response;
    dummy: CARDINAL;
    hdrView: BytesView;
    arenaMk: CARDINAL;
  BEGIN
    CASE hdr.ftype OF
      FrameSettings:
        IF (hdr.flags = FlagAck) THEN
          (* SETTINGS ACK from client *)
          cp^.settingsAckRecvd := TRUE;
        ELSE
          (* Client SETTINGS — decode and apply *)
          InitDefaultSettings(s);
          DecodeSettings(payload, s, ok);
          IF ok THEN
            cp^.remoteSettings := s;
            (* Update connection send window if initial window changed *)
            cp^.connSendWindow := INTEGER(s.initialWindowSize);
            (* Send SETTINGS ACK *)
            EncodeSettingsAck(cp^.writeBuf);
            IF cp^.phase = CpSettings THEN
              cp^.phase := CpOpen;
              (* Cancel handshake timeout — connection is now operational *)
              IF (cp^.hsTimerId >= 0) AND (cp^.loop # NIL) THEN
                EventLoop.CancelTimer(cp^.loop, cp^.hsTimerId);
                cp^.hsTimerId := -1;
              END;
            END;
          ELSE
            SendGoaway(cp, ErrProtocol);
          END;
        END;

    | FramePing:
        IF payload.len # 8 THEN
          SendGoaway(cp, ErrFrameSize);
        ELSIF (hdr.flags = FlagAck) THEN
          (* PING ACK — ignore *)
        ELSE
          (* Echo with ACK flag *)
          EncodePing(cp^.writeBuf, payload, TRUE);
        END;

    | FrameGoaway:
        DecodeGoaway(payload, lastStream, errorCode, ok);
        IF ok THEN
          cp^.goawayRecvd := TRUE;
          cp^.phase := CpGoaway;
        END;

    | FrameWindowUpdate:
        DecodeWindowUpdate(payload, increment, ok);
        IF ok AND (increment > 0) THEN
          IF hdr.streamId = 0 THEN
            (* Connection-level window update — peer is granting us more send budget *)
            cp^.connSendWindow := cp^.connSendWindow + INTEGER(increment);
          ELSE
            (* Stream-level window update *)
            slotIdx := FindSlot(cp^.slots, hdr.streamId);
            IF slotIdx < MaxStreamSlots THEN
              UpdateSendWindow(cp^.slots[slotIdx].stream, increment);
            END;
          END;
        ELSIF NOT ok THEN
          SendGoaway(cp, ErrProtocol);
        ELSIF increment = 0 THEN
          IF hdr.streamId = 0 THEN
            SendGoaway(cp, ErrProtocol);
          ELSE
            SendRstStream(cp, hdr.streamId, ErrProtocol);
          END;
        END;

    | FrameRstStream:
        DecodeRstStream(payload, errorCode, ok);
        IF ok THEN
          slotIdx := FindSlot(cp^.slots, hdr.streamId);
          IF slotIdx < MaxStreamSlots THEN
            IF (cp^.slots[slotIdx].phase = PhHeld) AND (HeldClosed # NIL) THEN
              HeldClosed(cp^.id, hdr.streamId);
            END;
            IF cp^.slots[slotIdx].phase = PhDeferred THEN
              (* Worker thread is still processing — just mark done.
                 CompletionQueue callback will discard the stale response. *)
              cp^.slots[slotIdx].phase := PhDone;
            ELSE
              SlotFree(cp^.slots[slotIdx]);
            END;
            IF cp^.numActive > 0 THEN
              DEC(cp^.numActive);
            END;
          END;
        END;

    | FrameHeaders:
        IF cp^.phase # CpOpen THEN
          SendGoaway(cp, ErrProtocol);
          RETURN;
        END;
        IF hdr.streamId = 0 THEN
          SendGoaway(cp, ErrProtocol);
          RETURN;
        END;
        (* Client streams must be odd *)
        IF (hdr.streamId MOD 2) = 0 THEN
          SendGoaway(cp, ErrProtocol);
          RETURN;
        END;
        (* Stream ID must be greater than last seen *)
        IF hdr.streamId <= cp^.lastPeerStream THEN
          SendGoaway(cp, ErrProtocol);
          RETURN;
        END;
        cp^.lastPeerStream := hdr.streamId;

        (* Enforce advertised max concurrent streams limit *)
        IF cp^.numActive >= cp^.localSettings.maxConcurrentStreams THEN
          SendRstStream(cp, hdr.streamId, 7); (* REFUSED_STREAM *)
          RETURN;
        END;

        (* Allocate a stream slot *)
        slotIdx := AllocSlot(cp^.slots, hdr.streamId,
                             cp^.remoteSettings.initialWindowSize,
                             ADR(cp^.streamTable));
        IF slotIdx >= MaxStreamSlots THEN
          SendRstStream(cp, hdr.streamId, 7); (* REFUSED_STREAM *)
          RETURN;
        END;
        INC(cp^.numActive);

        endStream := (hdr.flags DIV FlagEndStream) MOD 2 = 1;
        endHeaders := (hdr.flags DIV FlagEndHeaders) MOD 2 = 1;

        IF NOT endHeaders THEN
          (* We don't support CONTINUATION — require END_HEADERS *)
          SendGoaway(cp, ErrProtocol);
          RETURN;
        END;

        (* Decode HPACK header block *)
        arenaMk := ArenaMark(cp^.arena);
        DecodeHeaderBlock(payload, cp^.dynDec,
                          decoded, 16, numDecoded, ok);
        IF NOT ok THEN
          ArenaResetTo(cp^.arena, arenaMk);
          SendRstStream(cp, hdr.streamId, ErrProtocol);
          SlotFree(cp^.slots[slotIdx]);
          IF cp^.numActive > 0 THEN
            DEC(cp^.numActive);
          END;
          RETURN;
        END;

        (* Assemble headers into request *)
        cp^.slots[slotIdx].req.streamId := hdr.streamId;
        cp^.slots[slotIdx].req.connId := cp^.id;
        ok := AssembleHeaders(cp^.slots[slotIdx],
                              decoded, numDecoded, endStream);
        ArenaResetTo(cp^.arena, arenaMk);

        IF NOT ok THEN
          SendRstStream(cp, hdr.streamId, ErrProtocol);
          SlotFree(cp^.slots[slotIdx]);
          IF cp^.numActive > 0 THEN
            DEC(cp^.numActive);
          END;
          RETURN;
        END;

        (* If END_STREAM, dispatch immediately *)
        IF endStream THEN
          DispatchRequest(cp, slotIdx);
        END;

    | FrameData:
        IF hdr.streamId = 0 THEN
          SendGoaway(cp, ErrProtocol);
          RETURN;
        END;
        slotIdx := FindSlot(cp^.slots, hdr.streamId);
        IF slotIdx >= MaxStreamSlots THEN
          SendGoaway(cp, ErrProtocol);
          RETURN;
        END;

        endStream := (hdr.flags DIV FlagEndStream) MOD 2 = 1;

        (* Consume from connection receive window *)
        IF payload.len > 0 THEN
          IF INTEGER(payload.len) > cp^.connRecvWindow THEN
            SendGoaway(cp, ErrFlowControl);
            RETURN;
          END;
          cp^.connRecvWindow := cp^.connRecvWindow - INTEGER(payload.len);

          (* Send WINDOW_UPDATE to replenish connection window *)
          SendWindowUpdate(cp, 0, payload.len);
          cp^.connRecvWindow := cp^.connRecvWindow + INTEGER(payload.len);

          (* Send WINDOW_UPDATE for the stream *)
          SendWindowUpdate(cp, hdr.streamId, payload.len);
        END;

        ok := AccumulateData(cp^.slots[slotIdx], payload, endStream);
        IF NOT ok THEN
          SendRstStream(cp, hdr.streamId, ErrProtocol);
          SlotFree(cp^.slots[slotIdx]);
          IF cp^.numActive > 0 THEN
            DEC(cp^.numActive);
          END;
          RETURN;
        END;

        IF endStream THEN
          DispatchRequest(cp, slotIdx);
        END;

    | FramePriority:
        (* Ignore PRIORITY frames per RFC 9113 *)

    | FrameContinuation:
        (* We require END_HEADERS on HEADERS frames *)
        SendGoaway(cp, ErrProtocol);

    ELSE
      (* Unknown frame type — ignore per spec *)
    END;
  END ProcessFrame;

  (* ── Request dispatch ───────────────────────────────── *)

  PROCEDURE DispatchRequest(cp: ConnPtr; slotIdx: CARDINAL);
  VAR
    resp: Response;
    dummy: CARDINAL;
  BEGIN
    InitResponse(resp);

    (* Set connPtr before dispatch so handlers can access connection *)
    cp^.slots[slotIdx].req.connPtr := ADDRESS(cp);

    (* The server's router and middleware are accessed through
       the server back-pointer.  We call the dispatch procedure
       that Http2Server exports for this purpose. *)
    ServerDispatch(cp^.server,
                   cp^.slots[slotIdx].req, resp);

    (* If handler put stream into held state (SSE), skip response/cleanup *)
    IF cp^.slots[slotIdx].phase = PhHeld THEN
      FreeResponse(resp);
      RETURN;
    END;

    (* If handler deferred to worker thread, skip response/cleanup *)
    IF cp^.slots[slotIdx].phase = PhDeferred THEN
      FreeResponse(resp);
      RETURN;
    END;

    (* Guard: if handler left status 0, set 500 *)
    IF resp.status = 0 THEN
      resp.status := 500;
    END;

    (* Send response *)
    dummy := SendResponse(cp^.slots[slotIdx], resp,
                          cp^.dynEnc, cp^.writeBuf,
                          cp^.remoteSettings.maxFrameSize,
                          cp^.connSendWindow);

    FreeResponse(resp);

    (* Clean up slot if done *)
    IF cp^.slots[slotIdx].endSent THEN
      SlotFree(cp^.slots[slotIdx]);
      IF cp^.numActive > 0 THEN
        DEC(cp^.numActive);
      END;
    END;
  END DispatchRequest;

  (* ── Main I/O processing ────────────────────────────── *)

  PROCEDURE ProcessReadBuf(cp: ConnPtr);
  VAR
    v: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
    payloadView: BytesView;
    needed: CARDINAL;
  BEGIN
    (* Phase: waiting for client connection preface *)
    IF cp^.phase = CpPreface THEN
      IF cp^.readBuf.len < PrefaceLen THEN
        RETURN;
      END;
      v := AsView(cp^.readBuf);
      v.len := PrefaceLen;
      IF NOT CheckPreface(v) THEN
        SendGoaway(cp, ErrProtocol);
        RETURN;
      END;
      cp^.prefaceRecvd := TRUE;
      (* Remove preface bytes from readBuf *)
      ShiftBuf(cp^.readBuf, PrefaceLen);

      (* Send our SETTINGS *)
      EncodeSettings(cp^.writeBuf, cp^.localSettings);
      cp^.phase := CpSettings;
    END;

    (* Process frames *)
    LOOP
      IF cp^.phase >= CpClosed THEN
        EXIT;
      END;

      (* Try to parse frame header *)
      IF NOT cp^.haveHeader THEN
        IF cp^.readBuf.len < FrameHeaderSize THEN
          EXIT;
        END;
        v := AsView(cp^.readBuf);
        v.len := FrameHeaderSize;
        DecodeHeader(v, cp^.curHeader, ok);
        IF NOT ok THEN
          SendGoaway(cp, ErrProtocol);
          EXIT;
        END;
        cp^.haveHeader := TRUE;
      END;

      (* Wait for full payload *)
      needed := FrameHeaderSize + cp^.curHeader.length;
      IF cp^.readBuf.len < needed THEN
        EXIT;
      END;

      (* Extract payload view *)
      payloadView.base := ADDRESS(LONGCARD(cp^.readBuf.data) + LONGCARD(FrameHeaderSize));
      payloadView.len := cp^.curHeader.length;

      (* Process the frame *)
      ProcessFrame(cp, cp^.curHeader, payloadView);

      (* Consume the frame from readBuf *)
      ShiftBuf(cp^.readBuf, needed);
      cp^.haveHeader := FALSE;
    END;
  END ProcessReadBuf;

  (* Shift buffer: remove first n bytes by moving remaining to front *)
  PROCEDURE ShiftBuf(VAR b: Buf; n: CARDINAL);
  VAR
    remaining, i: CARDINAL;
    src, dst: ADDRESS;
    sp, dp: POINTER TO CHAR;
  BEGIN
    IF n >= b.len THEN
      Clear(b);
      RETURN;
    END;
    remaining := b.len - n;
    src := ADDRESS(LONGCARD(b.data) + LONGCARD(n));
    dst := b.data;
    (* Byte-by-byte copy; safe for overlapping *)
    FOR i := 0 TO remaining - 1 DO
      sp := ADDRESS(LONGCARD(src) + LONGCARD(i));
      dp := ADDRESS(LONGCARD(dst) + LONGCARD(i));
      dp^ := sp^;
    END;
    b.len := remaining;
  END ShiftBuf;

  (* ── EventLoop callback ────────────────────────────── *)

  PROCEDURE ConnOnEvent(fd, events: INTEGER; user: ADDRESS);
  VAR
    cp: ConnPtr;
    tmpBuf: ARRAY [0..4095] OF CHAR;
    got: INTEGER;
    st: TLS.Status;
    v: BytesView;
    evSt: EventLoop.Status;
  BEGIN
    cp := ConnPtr(user);
    IF cp = NIL THEN RETURN END;
    IF cp^.phase >= CpClosed THEN
      (* Already closed — trigger cleanup if callback set *)
      IF ConnCleanup # NIL THEN
        ConnCleanup(cp^.server, cp);
      END;
      RETURN;
    END;

    (* Test connections (no TLS) — data fed via ConnFeedBytes *)
    IF cp^.tlsSess = NIL THEN
      ProcessReadBuf(cp);
      RETURN;
    END;

    (* Non-blocking TLS handshake retry *)
    IF cp^.phase = CpHandshaking THEN
      st := TLS.Handshake(cp^.tlsSess);
      IF st = TLS.OK THEN
        (* Handshake complete — transition to preface wait *)
        cp^.phase := CpPreface;
        IF cp^.loop # NIL THEN
          evSt := EventLoop.ModifyFd(cp^.loop, cp^.fd, EvRead);
        END;
        (* Fall through to read any data buffered during handshake.
           Edge-triggered kqueue won't re-fire for data already received. *)
      ELSIF st = TLS.WantRead THEN
        IF cp^.loop # NIL THEN
          evSt := EventLoop.ModifyFd(cp^.loop, cp^.fd, EvRead);
        END;
        RETURN;
      ELSIF st = TLS.WantWrite THEN
        IF cp^.loop # NIL THEN
          evSt := EventLoop.ModifyFd(cp^.loop, cp^.fd, EvRead + EvWrite);
        END;
        RETURN;
      ELSE
        (* Handshake failed — close *)
        cp^.phase := CpClosed;
        IF ConnCleanup # NIL THEN
          ConnCleanup(cp^.server, cp);
        END;
        RETURN;
      END;
    END;

    (* Flush any pending write data first *)
    IF cp^.writeBuf.len > cp^.writeOff THEN
      ConnFlush(cp);
    END;

    (* Read from TLS into readBuf *)
    LOOP
      st := TLS.Read(cp^.tlsSess, ADR(tmpBuf), 4096, got);
      IF st = TLS.OK THEN
        IF got > 0 THEN
          v.base := ADR(tmpBuf);
          v.len := CARDINAL(got);
          AppendView(cp^.readBuf, v);
        ELSE
          (* got=0 means EOF — peer closed *)
          cp^.phase := CpClosed;
          IF ConnCleanup # NIL THEN
            ConnCleanup(cp^.server, cp);
          END;
          RETURN;
        END;
      ELSIF (st = TLS.WantRead) OR (st = TLS.WantWrite) THEN
        EXIT;
      ELSE
        (* TLS error or connection closed *)
        cp^.phase := CpClosed;
        IF ConnCleanup # NIL THEN
          ConnCleanup(cp^.server, cp);
        END;
        RETURN;
      END;
    END;

    ProcessReadBuf(cp);
    ConnFlush(cp);

    (* Check if processing moved us to closed/goaway state *)
    IF cp^.phase >= CpClosed THEN
      IF ConnCleanup # NIL THEN
        ConnCleanup(cp^.server, cp);
      END;
      RETURN;
    END;

    (* EV_CLEAR (edge-triggered) kqueue: the read event fires once for
       data+EOF combined.  After TLS.Read drains the SSL buffer it
       returns WantRead, but the peer may already have closed.
       If the poller reported EvHup (EV_EOF), close the connection now
       because kevent will NOT fire again for this fd. *)
    IF (CARDINAL(events) DIV EvHup) MOD 2 = 1 THEN
      cp^.phase := CpClosed;
      IF ConnCleanup # NIL THEN
        ConnCleanup(cp^.server, cp);
      END;
    END;
  END ConnOnEvent;

  (* ── Drain and close ────────────────────────────────── *)

  PROCEDURE ConnDrain(cp: ConnPtr);
  BEGIN
    IF cp = NIL THEN RETURN END;
    IF cp^.goawaySent THEN RETURN END;
    SendGoaway(cp, ErrNoError);
  END ConnDrain;

  PROCEDURE ConnClose(cp: ConnPtr);
  VAR
    i: CARDINAL;
    dummy: TLS.Status;
  BEGIN
    IF cp = NIL THEN RETURN END;

    (* Cancel handshake timer if still active *)
    IF (cp^.hsTimerId >= 0) AND (cp^.loop # NIL) THEN
      EventLoop.CancelTimer(cp^.loop, cp^.hsTimerId);
      cp^.hsTimerId := -1;
    END;

    (* Destroy TLS session *)
    IF cp^.tlsSess # NIL THEN
      dummy := TLS.SessionDestroy(cp^.tlsSess);
    END;

    (* Notify for any held streams before freeing *)
    FOR i := 0 TO MaxStreamSlots - 1 DO
      IF (cp^.slots[i].active) AND (cp^.slots[i].phase = PhHeld)
         AND (HeldClosed # NIL) THEN
        HeldClosed(cp^.id, cp^.slots[i].stream.id);
      END;
    END;

    (* Count deferred slots — mark them PhDone so completion callback
       knows the connection is dead, but do NOT free the ConnRec yet *)
    cp^.deferredCount := 0;
    FOR i := 0 TO MaxStreamSlots - 1 DO
      IF (cp^.slots[i].active) AND (cp^.slots[i].phase = PhDeferred) THEN
        cp^.slots[i].phase := PhDone;
        INC(cp^.deferredCount);
      END;
    END;

    (* Free stream slots — free ALL slots, not just active ones,
       because idle slots still hold a req.body buffer from SlotInit *)
    FOR i := 0 TO MaxStreamSlots - 1 DO
      FreeRequest(cp^.slots[i].req);
    END;

    (* Free I/O buffers *)
    Free(cp^.readBuf);
    Free(cp^.writeBuf);

    (* Free arena *)
    IF cp^.arenaBase # NIL THEN
      DEALLOCATE(cp^.arenaBase, ArenaSize);
      cp^.arenaBase := NIL;
    END;

    cp^.phase := CpClosed;

    (* If deferred work items are outstanding, delay DEALLOCATE.
       The CompletionQueue callback will DEALLOCATE after draining. *)
    IF cp^.deferredCount = 0 THEN
      DEALLOCATE(cp, TSIZE(ConnRec));
    END;
  END ConnClose;

  (* ── Flush ──────────────────────────────────────────── *)

  PROCEDURE ConnFlush(cp: ConnPtr);
  VAR
    st: TLS.Status;
    sent: INTEGER;
    dataAddr: ADDRESS;
    remaining: INTEGER;
    evSt: EventLoop.Status;
  BEGIN
    IF cp = NIL THEN RETURN END;
    (* Test connections: leave data in writeBuf for test harness *)
    IF cp^.tlsSess = NIL THEN RETURN END;

    WHILE cp^.writeOff < cp^.writeBuf.len DO
      remaining := INTEGER(cp^.writeBuf.len - cp^.writeOff);
      dataAddr := ADDRESS(LONGCARD(cp^.writeBuf.data) + LONGCARD(cp^.writeOff));
      st := TLS.Write(cp^.tlsSess, dataAddr, remaining, sent);
      IF st = TLS.OK THEN
        cp^.writeOff := cp^.writeOff + CARDINAL(sent);
      ELSIF (st = TLS.WantRead) OR (st = TLS.WantWrite) THEN
        (* Socket buffer full — watch for EvWrite to resume *)
        IF cp^.loop # NIL THEN
          evSt := EventLoop.ModifyFd(cp^.loop, cp^.fd, EvRead + EvWrite);
        END;
        RETURN;
      ELSE
        (* TLS write error *)
        cp^.phase := CpClosed;
        RETURN;
      END;
    END;

    (* All data flushed — clear buffer and revert to EvRead only *)
    IF cp^.writeOff > 0 THEN
      Clear(cp^.writeBuf);
      cp^.writeOff := 0;
      IF cp^.loop # NIL THEN
        evSt := EventLoop.ModifyFd(cp^.loop, cp^.fd, EvRead);
      END;
    END;
  END ConnFlush;

  (* ── Server dispatch bridge ─────────────────────────── *)

  (* This procedure is called by ProcessFrame to dispatch a request
     through the server's router and middleware chain.
     It's implemented in Http2Server.mod and exported for our use.
     We declare it as a module-level variable that Http2Server sets. *)

  PROCEDURE SetServerDispatch(p: DispatchProc);
  BEGIN
    ServerDispatch := p;
  END SetServerDispatch;

  PROCEDURE SetConnCleanup(p: CleanupProc);
  BEGIN
    ConnCleanup := p;
  END SetConnCleanup;

  PROCEDURE SetHeldStreamClosed(p: HeldClosedProc);
  BEGIN
    HeldClosed := p;
  END SetHeldStreamClosed;

  VAR
    ServerDispatch: DispatchProc;
    ConnCleanup: CleanupProc;
    HeldClosed: HeldClosedProc;

BEGIN
  ServerDispatch := NIL;
  ConnCleanup := NIL;
  HeldClosed := NIL;
END Http2ServerConn.
