IMPLEMENTATION MODULE Http2Conn;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Fsm IMPORT StepStatus;
FROM ByteBuf IMPORT BytesView, Buf, Init, Free, Clear, AsView,
                    AppendView;
FROM Http2Types IMPORT FrameHeader, Settings, FrameHeaderSize,
                       FrameData, FrameHeaders, FrameSettings,
                       FramePing, FrameGoaway, FrameWindowUpdate,
                       FrameRstStream, FrameContinuation,
                       FlagAck, FlagEndStream, FlagEndHeaders,
                       ConnectionStreamId,
                       DefaultWindowSize, DefaultHeaderTableSize,
                       StIdle, StOpen, StHalfClosedLocal,
                       StHalfClosedRemote, StClosed,
                       EvSendH, EvSendHES, EvSendES, EvSendRst,
                       EvRecvH, EvRecvHES, EvRecvES, EvRecvRst,
                       ErrNoError, ErrProtocol, ErrFlowControl,
                       ErrFrameSize, ErrStreamClosed;
FROM Http2Frame IMPORT WritePreface, EncodeSettings, EncodeSettingsAck,
                       EncodePing, EncodeGoaway, EncodeWindowUpdate,
                       DecodeSettings, DecodeGoaway,
                       DecodeWindowUpdate, DecodeRstStream;
FROM Http2Stream IMPORT H2Stream, StreamTransTable, InitStreamTable,
                        InitStream, StreamStep, StreamState,
                        UpdateSendWindow, ConsumeRecvWindow, IsClosed;
FROM Http2Hpack IMPORT DynInit;

(* ── Lifecycle ─────────────────────────────────────────── *)

PROCEDURE InitConn(VAR c: H2Conn);
VAR i: CARDINAL;
BEGIN
  Http2Types.InitDefaultSettings(c.localSettings);
  Http2Types.InitDefaultSettings(c.remoteSettings);
  c.connSendWindow := VAL(INTEGER, DefaultWindowSize);
  c.connRecvWindow := VAL(INTEGER, DefaultWindowSize);
  c.nextStreamId := 1;
  c.lastPeerStream := 0;
  c.goawayCode := 0;
  c.goawaySent := FALSE;
  c.goawayRecv := FALSE;
  c.numActive := 0;
  i := 0;
  WHILE i < MaxStreams DO
    c.streamUsed[i] := FALSE;
    INC(i)
  END;
  InitStreamTable(c.streamTable);
  DynInit(c.dynTableEnc, DefaultHeaderTableSize);
  DynInit(c.dynTableDec, DefaultHeaderTableSize);
  Init(c.outBuf, 4096)
END InitConn;

PROCEDURE FreeConn(VAR c: H2Conn);
BEGIN
  Free(c.outBuf)
END FreeConn;

(* ── Connection preface ────────────────────────────────── *)

PROCEDURE SendPreface(VAR c: H2Conn);
BEGIN
  WritePreface(c.outBuf);
  EncodeSettings(c.outBuf, c.localSettings)
END SendPreface;

(* ── Stream management ─────────────────────────────────── *)

PROCEDURE FindStream(VAR c: H2Conn; streamId: CARDINAL): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < MaxStreams DO
    IF c.streamUsed[i] AND (c.streams[i].id = streamId) THEN
      RETURN i
    END;
    INC(i)
  END;
  RETURN MaxStreams
END FindStream;

PROCEDURE FindFreeSlot(VAR c: H2Conn): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < MaxStreams DO
    IF NOT c.streamUsed[i] THEN RETURN i END;
    INC(i)
  END;
  RETURN MaxStreams
END FindFreeSlot;

PROCEDURE OpenStream(VAR c: H2Conn): CARDINAL;
VAR slot, sid: CARDINAL;
BEGIN
  IF c.goawayRecv THEN RETURN 0 END;
  slot := FindFreeSlot(c);
  IF slot = MaxStreams THEN RETURN 0 END;
  sid := c.nextStreamId;
  INC(c.nextStreamId, 2);  (* Client streams are odd *)
  InitStream(c.streams[slot], sid,
             c.remoteSettings.initialWindowSize,
             ADR(c.streamTable));
  c.streamUsed[slot] := TRUE;
  INC(c.numActive);
  RETURN sid
END OpenStream;

PROCEDURE ReleaseStream(VAR c: H2Conn; slot: CARDINAL);
BEGIN
  IF (slot < MaxStreams) AND c.streamUsed[slot] THEN
    c.streamUsed[slot] := FALSE;
    IF c.numActive > 0 THEN DEC(c.numActive) END
  END
END ReleaseStream;

(* ── Frame processing ──────────────────────────────────── *)

PROCEDURE ProcessSettingsFrame(VAR c: H2Conn;
                               hdr: FrameHeader;
                               payload: BytesView;
                               VAR ok: BOOLEAN);
VAR s: Settings;
BEGIN
  IF (hdr.flags MOD 2) = 1 THEN
    (* ACK: no payload expected *)
    IF hdr.length # 0 THEN ok := FALSE END;
    RETURN
  END;
  IF (hdr.length MOD 6) # 0 THEN
    ok := FALSE;
    RETURN
  END;
  s := c.remoteSettings;
  DecodeSettings(payload, s, ok);
  IF ok THEN
    ApplyRemoteSettings(c, s);
    EncodeSettingsAck(c.outBuf)
  END
END ProcessSettingsFrame;

PROCEDURE ProcessPingFrame(VAR c: H2Conn;
                           hdr: FrameHeader;
                           payload: BytesView;
                           VAR ok: BOOLEAN);
BEGIN
  IF hdr.length # 8 THEN ok := FALSE; RETURN END;
  IF hdr.streamId # ConnectionStreamId THEN ok := FALSE; RETURN END;
  IF (hdr.flags MOD 2) = 1 THEN
    (* ACK: nothing to do *)
    RETURN
  END;
  (* Send PING ACK with same opaque data *)
  EncodePing(c.outBuf, payload, TRUE)
END ProcessPingFrame;

PROCEDURE ProcessGoawayFrame(VAR c: H2Conn;
                             payload: BytesView;
                             VAR ok: BOOLEAN);
VAR lastId, errCode: CARDINAL;
BEGIN
  DecodeGoaway(payload, lastId, errCode, ok);
  IF NOT ok THEN RETURN END;
  c.goawayRecv := TRUE;
  c.lastPeerStream := lastId;
  c.goawayCode := errCode
END ProcessGoawayFrame;

PROCEDURE ProcessWindowUpdateFrame(VAR c: H2Conn;
                                   hdr: FrameHeader;
                                   payload: BytesView;
                                   VAR ok: BOOLEAN);
VAR increment, slot: CARDINAL;
BEGIN
  DecodeWindowUpdate(payload, increment, ok);
  IF NOT ok THEN RETURN END;
  IF increment = 0 THEN ok := FALSE; RETURN END;
  IF hdr.streamId = ConnectionStreamId THEN
    UpdateConnSendWindow(c, increment)
  ELSE
    slot := FindStream(c, hdr.streamId);
    IF slot < MaxStreams THEN
      Http2Stream.UpdateSendWindow(c.streams[slot], increment)
    END
  END
END ProcessWindowUpdateFrame;

PROCEDURE ProcessRstStreamFrame(VAR c: H2Conn;
                                hdr: FrameHeader;
                                payload: BytesView;
                                VAR ok: BOOLEAN);
VAR errCode, slot: CARDINAL;
    status: StepStatus;
BEGIN
  IF hdr.length # 4 THEN ok := FALSE; RETURN END;
  IF hdr.streamId = ConnectionStreamId THEN ok := FALSE; RETURN END;
  DecodeRstStream(payload, errCode, ok);
  IF NOT ok THEN RETURN END;
  slot := FindStream(c, hdr.streamId);
  IF slot < MaxStreams THEN
    c.streams[slot].rstCode := errCode;
    StreamStep(c.streams[slot], EvRecvRst, status);
    IF IsClosed(c.streams[slot]) THEN
      ReleaseStream(c, slot)
    END
  END
END ProcessRstStreamFrame;

PROCEDURE ProcessDataFrame(VAR c: H2Conn;
                           hdr: FrameHeader;
                           payload: BytesView;
                           VAR ok: BOOLEAN);
VAR slot: CARDINAL;
    status: StepStatus;
BEGIN
  IF hdr.streamId = ConnectionStreamId THEN
    ok := FALSE;
    RETURN
  END;
  slot := FindStream(c, hdr.streamId);
  IF slot >= MaxStreams THEN
    ok := FALSE;
    RETURN
  END;
  (* Consume from connection + stream recv windows *)
  IF NOT ConsumeRecvWindow(c.streams[slot], hdr.length) THEN
    ok := FALSE;
    RETURN
  END;
  c.connRecvWindow := c.connRecvWindow - VAL(INTEGER, hdr.length);
  (* Check END_STREAM flag *)
  IF (hdr.flags MOD 2) = 1 THEN
    StreamStep(c.streams[slot], EvRecvES, status);
    IF IsClosed(c.streams[slot]) THEN
      ReleaseStream(c, slot)
    END
  END
END ProcessDataFrame;

PROCEDURE ProcessHeadersFrame(VAR c: H2Conn;
                              hdr: FrameHeader;
                              payload: BytesView;
                              VAR ok: BOOLEAN);
VAR slot: CARDINAL;
    status: StepStatus;
    hasEndStream, hasEndHeaders: BOOLEAN;
    ev: CARDINAL;
BEGIN
  IF hdr.streamId = ConnectionStreamId THEN
    ok := FALSE;
    RETURN
  END;
  slot := FindStream(c, hdr.streamId);
  IF slot >= MaxStreams THEN
    (* Could be a new server-pushed stream; for now reject *)
    ok := FALSE;
    RETURN
  END;
  hasEndStream := ((hdr.flags MOD 2) = 1);
  hasEndHeaders := ((hdr.flags DIV 4) MOD 2 = 1);
  IF hasEndStream THEN
    ev := EvRecvHES
  ELSE
    ev := EvRecvH
  END;
  StreamStep(c.streams[slot], ev, status);
  IF IsClosed(c.streams[slot]) THEN
    ReleaseStream(c, slot)
  END
END ProcessHeadersFrame;

PROCEDURE ProcessFrame(VAR c: H2Conn;
                       hdr: FrameHeader;
                       payload: BytesView;
                       VAR ok: BOOLEAN);
BEGIN
  ok := TRUE;
  IF hdr.ftype = FrameSettings THEN
    ProcessSettingsFrame(c, hdr, payload, ok)
  ELSIF hdr.ftype = FramePing THEN
    ProcessPingFrame(c, hdr, payload, ok)
  ELSIF hdr.ftype = FrameGoaway THEN
    ProcessGoawayFrame(c, payload, ok)
  ELSIF hdr.ftype = FrameWindowUpdate THEN
    ProcessWindowUpdateFrame(c, hdr, payload, ok)
  ELSIF hdr.ftype = FrameRstStream THEN
    ProcessRstStreamFrame(c, hdr, payload, ok)
  ELSIF hdr.ftype = FrameData THEN
    ProcessDataFrame(c, hdr, payload, ok)
  ELSIF hdr.ftype = FrameHeaders THEN
    ProcessHeadersFrame(c, hdr, payload, ok)
  END
  (* Other frame types (PRIORITY, CONTINUATION, etc.) are ignored *)
END ProcessFrame;

(* ── Flow control ──────────────────────────────────────── *)

PROCEDURE UpdateConnSendWindow(VAR c: H2Conn; increment: CARDINAL);
BEGIN
  c.connSendWindow := c.connSendWindow + VAL(INTEGER, increment)
END UpdateConnSendWindow;

PROCEDURE ConsumeConnSendWindow(VAR c: H2Conn; n: CARDINAL): BOOLEAN;
VAR needed: INTEGER;
BEGIN
  needed := VAL(INTEGER, n);
  IF c.connSendWindow < needed THEN RETURN FALSE END;
  c.connSendWindow := c.connSendWindow - needed;
  RETURN TRUE
END ConsumeConnSendWindow;

(* ── Settings ──────────────────────────────────────────── *)

PROCEDURE ApplyRemoteSettings(VAR c: H2Conn; s: Settings);
BEGIN
  c.remoteSettings := s
END ApplyRemoteSettings;

(* ── Output ────────────────────────────────────────────── *)

PROCEDURE GetOutput(VAR c: H2Conn): BytesView;
BEGIN
  RETURN AsView(c.outBuf)
END GetOutput;

PROCEDURE ClearOutput(VAR c: H2Conn);
BEGIN
  Clear(c.outBuf)
END ClearOutput;

END Http2Conn.
