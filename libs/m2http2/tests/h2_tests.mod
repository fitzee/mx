MODULE h2_tests;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM ByteBuf IMPORT BytesView, Buf, Init, Free, Clear, AsView,
                    AppendByte, ViewGetByte, GetByte;
FROM Codec IMPORT Reader, Writer, InitReader, InitWriter,
                  ReadU8, WriteU8;
FROM Fsm IMPORT StepStatus;

FROM Http2Types IMPORT FrameHeader, Settings,
                       FrameData, FrameHeaders, FrameSettings,
                       FramePing, FrameGoaway, FrameWindowUpdate,
                       FrameRstStream, FrameHeaderSize,
                       FlagEndStream, FlagAck, FlagEndHeaders,
                       ConnectionStreamId, PrefaceLen,
                       DefaultHeaderTableSize, DefaultInitialWindowSize,
                       DefaultMaxFrameSize, DefaultEnablePush,
                       ErrNoError, ErrProtocol, ErrCancel,
                       HeaderEntry, MaxHeaders,
                       StIdle, StOpen, StHalfClosedLocal,
                       StHalfClosedRemote, StClosed,
                       NumStreamStates, NumStreamEvents,
                       EvSendH, EvSendHES, EvSendES, EvSendRst,
                       EvRecvH, EvRecvHES, EvRecvES, EvRecvRst;

FROM Http2Frame IMPORT DecodeHeader, EncodeHeader,
                       WritePreface, CheckPreface,
                       DecodeSettings, EncodeSettings, EncodeSettingsAck,
                       EncodePing, EncodeGoaway, DecodeGoaway,
                       EncodeWindowUpdate, DecodeWindowUpdate,
                       EncodeRstStream, DecodeRstStream,
                       EncodeDataHeader, EncodeHeadersHeader;

FROM Http2Hpack IMPORT EncodeInt, DecodeInt,
                       StaticLookup, StaticFind,
                       DynTable, DynInit, DynInsert, DynLookup,
                       DynResize, DynCount,
                       DecodeHeaderBlock, EncodeHeaderBlock;

FROM Http2Stream IMPORT H2Stream, StreamTransTable,
                        InitStreamTable, InitStream, StreamStep,
                        StreamState, IsClosed,
                        ConsumeSendWindow, UpdateSendWindow;

FROM Http2Conn IMPORT H2Conn, MaxStreams,
                      InitConn, FreeConn, SendPreface,
                      OpenStream, FindStream,
                      ProcessFrame, GetOutput, ClearOutput,
                      UpdateConnSendWindow, ConsumeConnSendWindow;

FROM Http2TestUtil IMPORT BuildFrame, BuildSettingsFrame,
                          BuildSettingsAckFrame, BuildPingFrame,
                          BuildGoawayFrame, BuildWindowUpdateFrame,
                          BuildRstStreamFrame,
                          ReadFrameHeader, ReadFramePayload;

VAR
  passed, failed, total: CARDINAL;

PROCEDURE Check(label: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("FAIL: ");
    WriteString(label);
    WriteLn
  END
END Check;

(* ══════════════════════════════════════════════════════════ *)
(* 1. Frame header encode/decode                             *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestFrameHeader;
VAR b: Buf;
    v: BytesView;
    hdr, hdr2: FrameHeader;
    ok: BOOLEAN;
BEGIN
  Init(b, 256);
  (* Encode a frame header *)
  hdr.length := 100;
  hdr.ftype := FrameData;
  hdr.flags := FlagEndStream;
  hdr.streamId := 7;
  EncodeHeader(b, hdr);
  Check("frame.hdr: len=9", b.len = 9);
  v := AsView(b);
  DecodeHeader(v, hdr2, ok);
  Check("frame.hdr: decode ok", ok);
  Check("frame.hdr: length", hdr2.length = 100);
  Check("frame.hdr: type", hdr2.ftype = FrameData);
  Check("frame.hdr: flags", hdr2.flags = FlagEndStream);
  Check("frame.hdr: streamId", hdr2.streamId = 7);

  (* Test with larger values *)
  Clear(b);
  hdr.length := 16384;
  hdr.ftype := FrameHeaders;
  hdr.flags := FlagEndHeaders + FlagEndStream;
  hdr.streamId := 1000001;
  EncodeHeader(b, hdr);
  v := AsView(b);
  DecodeHeader(v, hdr2, ok);
  Check("frame.hdr2: ok", ok);
  Check("frame.hdr2: length", hdr2.length = 16384);
  Check("frame.hdr2: type", hdr2.ftype = FrameHeaders);
  Check("frame.hdr2: flags", hdr2.flags = FlagEndHeaders + FlagEndStream);
  Check("frame.hdr2: streamId", hdr2.streamId = 1000001);

  (* Too short *)
  v.len := 5;
  DecodeHeader(v, hdr2, ok);
  Check("frame.hdr: too short", NOT ok);
  Free(b)
END TestFrameHeader;

(* ══════════════════════════════════════════════════════════ *)
(* 2. Connection preface                                     *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestPreface;
VAR b: Buf;
    v: BytesView;
BEGIN
  Init(b, 64);
  WritePreface(b);
  Check("preface: len=24", b.len = PrefaceLen);
  v := AsView(b);
  Check("preface: valid", CheckPreface(v));
  (* Corrupt one byte *)
  AppendByte(b, 0);  (* extend *)
  v := AsView(b);
  Check("preface: still valid with extra", CheckPreface(v));
  (* Too short *)
  v.len := 10;
  Check("preface: too short", NOT CheckPreface(v));
  Free(b)
END TestPreface;

(* ══════════════════════════════════════════════════════════ *)
(* 3. SETTINGS frame                                         *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestSettings;
VAR b: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    s: Settings;
    ok: BOOLEAN;
BEGIN
  Init(b, 256);
  Http2Types.InitDefaultSettings(s);
  s.maxFrameSize := 32768;
  s.initialWindowSize := 131072;
  EncodeSettings(b, s);
  Check("settings: frame len", b.len = 9 + 36);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("settings: decode hdr", ok);
  Check("settings: type", hdr.ftype = FrameSettings);
  Check("settings: payload len", hdr.length = 36);
  Check("settings: stream 0", hdr.streamId = 0);
  (* Decode settings payload *)
  payload.base := VAL(ADDRESS,
    LONGCARD(v.base) + LONGCARD(FrameHeaderSize));
  payload.len := hdr.length;
  Http2Types.InitDefaultSettings(s);  (* reset *)
  DecodeSettings(payload, s, ok);
  Check("settings: decode ok", ok);
  Check("settings: maxFrameSize", s.maxFrameSize = 32768);
  Check("settings: initialWindowSize", s.initialWindowSize = 131072);

  (* Settings ACK *)
  Clear(b);
  EncodeSettingsAck(b);
  Check("settings_ack: len=9", b.len = 9);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("settings_ack: type", hdr.ftype = FrameSettings);
  Check("settings_ack: ack flag", (hdr.flags MOD 2) = 1);
  Check("settings_ack: no payload", hdr.length = 0);
  Free(b)
END TestSettings;

(* ══════════════════════════════════════════════════════════ *)
(* 4. PING frame                                             *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestPing;
VAR b, pingData: Buf;
    v: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
    i: CARDINAL;
BEGIN
  Init(b, 64);
  Init(pingData, 16);
  i := 0;
  WHILE i < 8 DO
    AppendByte(pingData, i + 1);
    INC(i)
  END;
  v := AsView(pingData);
  EncodePing(b, v, FALSE);
  Check("ping: frame len", b.len = 9 + 8);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("ping: type", hdr.ftype = FramePing);
  Check("ping: no ack", (hdr.flags MOD 2) = 0);
  Check("ping: payload 8", hdr.length = 8);
  (* Verify opaque data preserved *)
  Check("ping: data[0]", GetByte(b, 9) = 1);
  Check("ping: data[7]", GetByte(b, 16) = 8);

  (* PING ACK *)
  Clear(b);
  v := AsView(pingData);
  EncodePing(b, v, TRUE);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("ping_ack: ack flag", (hdr.flags MOD 2) = 1);
  Free(b);
  Free(pingData)
END TestPing;

(* ══════════════════════════════════════════════════════════ *)
(* 5. GOAWAY frame                                           *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestGoaway;
VAR b: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    lastId, errCode: CARDINAL;
    ok: BOOLEAN;
BEGIN
  Init(b, 64);
  EncodeGoaway(b, 42, ErrProtocol);
  Check("goaway: len", b.len = 9 + 8);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("goaway: type", hdr.ftype = FrameGoaway);
  Check("goaway: stream 0", hdr.streamId = 0);
  payload.base := VAL(ADDRESS,
    LONGCARD(v.base) + LONGCARD(FrameHeaderSize));
  payload.len := hdr.length;
  DecodeGoaway(payload, lastId, errCode, ok);
  Check("goaway: decode ok", ok);
  Check("goaway: lastStreamId", lastId = 42);
  Check("goaway: errorCode", errCode = ErrProtocol);
  Free(b)
END TestGoaway;

(* ══════════════════════════════════════════════════════════ *)
(* 6. WINDOW_UPDATE frame                                    *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestWindowUpdate;
VAR b: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    inc: CARDINAL;
    ok: BOOLEAN;
BEGIN
  Init(b, 64);
  EncodeWindowUpdate(b, 3, 65536);
  Check("winup: len", b.len = 9 + 4);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("winup: type", hdr.ftype = FrameWindowUpdate);
  Check("winup: streamId", hdr.streamId = 3);
  payload.base := VAL(ADDRESS,
    LONGCARD(v.base) + LONGCARD(FrameHeaderSize));
  payload.len := hdr.length;
  DecodeWindowUpdate(payload, inc, ok);
  Check("winup: decode ok", ok);
  Check("winup: increment", inc = 65536);
  Free(b)
END TestWindowUpdate;

(* ══════════════════════════════════════════════════════════ *)
(* 7. RST_STREAM frame                                       *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestRstStream;
VAR b: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    errCode: CARDINAL;
    ok: BOOLEAN;
BEGIN
  Init(b, 64);
  EncodeRstStream(b, 5, ErrCancel);
  Check("rst: len", b.len = 9 + 4);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("rst: type", hdr.ftype = FrameRstStream);
  Check("rst: streamId", hdr.streamId = 5);
  payload.base := VAL(ADDRESS,
    LONGCARD(v.base) + LONGCARD(FrameHeaderSize));
  payload.len := hdr.length;
  DecodeRstStream(payload, errCode, ok);
  Check("rst: decode ok", ok);
  Check("rst: errorCode", errCode = ErrCancel);
  Free(b)
END TestRstStream;

(* ══════════════════════════════════════════════════════════ *)
(* 8. HPACK integer codec                                    *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestHpackInt;
VAR b: Buf;
    v: BytesView;
    pos, result: CARDINAL;
    ok: BOOLEAN;
    firstByte: CARDINAL;
BEGIN
  Init(b, 64);

  (* Small value: 10 with 5-bit prefix *)
  EncodeInt(b, 10, 5, 0);
  Check("hpack.int: small=1 byte", b.len = 1);
  v := AsView(b);
  firstByte := ViewGetByte(v, 0) MOD 32;
  pos := 1;
  result := DecodeInt(firstByte, 5, v, pos, ok);
  Check("hpack.int: small decode ok", ok);
  Check("hpack.int: small=10", result = 10);

  (* Exactly at boundary: 31 with 5-bit prefix *)
  Clear(b);
  EncodeInt(b, 31, 5, 0);
  Check("hpack.int: boundary bytes", b.len = 2);
  v := AsView(b);
  firstByte := ViewGetByte(v, 0) MOD 32;
  pos := 1;
  result := DecodeInt(firstByte, 5, v, pos, ok);
  Check("hpack.int: boundary ok", ok);
  Check("hpack.int: boundary=31", result = 31);

  (* Larger value: 1337 with 5-bit prefix (RFC example) *)
  Clear(b);
  EncodeInt(b, 1337, 5, 0);
  Check("hpack.int: 1337 bytes", b.len = 3);
  v := AsView(b);
  firstByte := ViewGetByte(v, 0) MOD 32;
  pos := 1;
  result := DecodeInt(firstByte, 5, v, pos, ok);
  Check("hpack.int: 1337 ok", ok);
  Check("hpack.int: 1337", result = 1337);

  (* Value 0 *)
  Clear(b);
  EncodeInt(b, 0, 7, 128);
  Check("hpack.int: zero=1 byte", b.len = 1);
  v := AsView(b);
  Check("hpack.int: zero first byte", ViewGetByte(v, 0) = 128);
  firstByte := ViewGetByte(v, 0) MOD 128;
  pos := 1;
  result := DecodeInt(firstByte, 7, v, pos, ok);
  Check("hpack.int: zero ok", ok);
  Check("hpack.int: zero=0", result = 0);

  (* Mask preserved: 42 with 6-bit prefix, mask=64 *)
  Clear(b);
  EncodeInt(b, 42, 6, 64);
  v := AsView(b);
  Check("hpack.int: mask byte", ViewGetByte(v, 0) = 64 + 42);

  Free(b)
END TestHpackInt;

(* ══════════════════════════════════════════════════════════ *)
(* 9. HPACK static table                                     *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestStaticTable;
VAR entry: HeaderEntry;
    ok: BOOLEAN;
    idx: CARDINAL;
BEGIN
  StaticLookup(2, entry, ok);
  Check("static: idx 2 ok", ok);
  Check("static: idx 2 name", (entry.name[0] = ":") AND
                               (entry.name[1] = "m"));
  Check("static: idx 2 nameLen", entry.nameLen = 7);
  Check("static: idx 2 value GET", (entry.value[0] = "G") AND
                                    (entry.value[1] = "E") AND
                                    (entry.value[2] = "T"));
  Check("static: idx 2 valLen", entry.valLen = 3);

  StaticLookup(4, entry, ok);
  Check("static: idx 4 :path /", ok AND (entry.nameLen = 5) AND
                                  (entry.valLen = 1));

  (* Out of bounds *)
  StaticLookup(0, entry, ok);
  Check("static: idx 0 fail", NOT ok);
  StaticLookup(62, entry, ok);
  Check("static: idx 62 fail", NOT ok);

  (* Find by name+value *)
  idx := StaticFind(":method", 7, "GET", 3, FALSE);
  Check("static.find: :method GET", idx = 2);

  idx := StaticFind(":method", 7, "POST", 4, FALSE);
  Check("static.find: :method POST", idx = 3);

  (* Name-only match *)
  idx := StaticFind(":status", 7, "999", 3, FALSE);
  Check("static.find: :status name match", idx = 8);

  idx := StaticFind("nonexistent", 11, "", 0, TRUE);
  Check("static.find: not found", idx = 0)
END TestStaticTable;

(* ══════════════════════════════════════════════════════════ *)
(* 10. HPACK dynamic table                                   *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestDynTable;
VAR dt: DynTable;
    entry: HeaderEntry;
    ok: BOOLEAN;
BEGIN
  DynInit(dt, 4096);
  Check("dyn: empty count", DynCount(dt) = 0);

  DynInsert(dt, "custom-key", 10, "custom-val", 10);
  Check("dyn: count=1", DynCount(dt) = 1);

  DynLookup(dt, 0, entry, ok);
  Check("dyn: lookup 0 ok", ok);
  Check("dyn: lookup 0 name", (entry.name[0] = "c") AND
                               (entry.nameLen = 10));
  Check("dyn: lookup 0 val", (entry.value[0] = "c") AND
                              (entry.valLen = 10));

  (* Insert second entry: newest is at index 0 *)
  DynInsert(dt, "another", 7, "value", 5);
  Check("dyn: count=2", DynCount(dt) = 2);
  DynLookup(dt, 0, entry, ok);
  Check("dyn: newest name", (entry.name[0] = "a") AND
                             (entry.nameLen = 7));

  DynLookup(dt, 1, entry, ok);
  Check("dyn: oldest name", (entry.name[0] = "c") AND
                             (entry.nameLen = 10));

  (* Out of bounds *)
  DynLookup(dt, 2, entry, ok);
  Check("dyn: oob fail", NOT ok);

  (* Resize to evict *)
  DynResize(dt, 50);  (* Only room for ~1 entry *)
  Check("dyn: resize count", DynCount(dt) <= 1);

  (* Insert oversized entry: should clear table *)
  DynResize(dt, 4096);
  DynInit(dt, 30);  (* Too small for any entry (32 overhead) *)
  DynInsert(dt, "x", 1, "y", 1);  (* size = 1+1+32 = 34 > 30 *)
  Check("dyn: oversized clears", DynCount(dt) = 0)
END TestDynTable;

(* ══════════════════════════════════════════════════════════ *)
(* 11. Stream FSM                                            *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestStreamFsm;
VAR table: StreamTransTable;
    s: H2Stream;
    status: StepStatus;
BEGIN
  InitStreamTable(table);

  (* Idle -> Open via SendH *)
  InitStream(s, 1, DefaultInitialWindowSize, ADR(table));
  Check("stream: idle", StreamState(s) = StIdle);
  StreamStep(s, EvSendH, status);
  Check("stream: sendH ok", status = Ok);
  Check("stream: open", StreamState(s) = StOpen);

  (* Open -> HalfClosedLocal via SendES *)
  StreamStep(s, EvSendES, status);
  Check("stream: sendES ok", status = Ok);
  Check("stream: half-closed-local", StreamState(s) = StHalfClosedLocal);

  (* HalfClosedLocal -> Closed via RecvES *)
  StreamStep(s, EvRecvES, status);
  Check("stream: recvES ok", status = Ok);
  Check("stream: closed", IsClosed(s));

  (* New stream: Idle -> HalfClosedLocal via SendHES *)
  InitStream(s, 3, DefaultInitialWindowSize, ADR(table));
  StreamStep(s, EvSendHES, status);
  Check("stream: sendHES ok", status = Ok);
  Check("stream: half-closed-local2", StreamState(s) = StHalfClosedLocal);

  (* RST_STREAM from any state *)
  InitStream(s, 5, DefaultInitialWindowSize, ADR(table));
  StreamStep(s, EvSendH, status);
  StreamStep(s, EvRecvRst, status);
  Check("stream: recvRst ok", status = Ok);
  Check("stream: rst->closed", IsClosed(s));

  (* Invalid transition: Closed + SendH *)
  StreamStep(s, EvSendH, status);
  Check("stream: closed no trans", status = NoTransition)
END TestStreamFsm;

(* ══════════════════════════════════════════════════════════ *)
(* 12. Stream flow control                                   *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestStreamFlowControl;
VAR table: StreamTransTable;
    s: H2Stream;
    status: StepStatus;
    ok: BOOLEAN;
BEGIN
  InitStreamTable(table);
  InitStream(s, 1, 100, ADR(table));
  StreamStep(s, EvSendH, status);

  ok := ConsumeSendWindow(s, 50);
  Check("flow: consume 50 ok", ok);
  ok := ConsumeSendWindow(s, 60);
  Check("flow: consume 60 fail", NOT ok);
  ok := ConsumeSendWindow(s, 50);
  Check("flow: consume last 50 ok", ok);
  ok := ConsumeSendWindow(s, 1);
  Check("flow: exhausted", NOT ok);
  UpdateSendWindow(s, 100);
  ok := ConsumeSendWindow(s, 100);
  Check("flow: after update ok", ok)
END TestStreamFlowControl;

(* ══════════════════════════════════════════════════════════ *)
(* 13. Connection lifecycle                                  *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestConnLifecycle;
VAR c: H2Conn;
    v: BytesView;
BEGIN
  InitConn(c);
  SendPreface(c);
  v := GetOutput(c);
  (* 24 bytes preface + 9+36 bytes settings = 69 *)
  Check("conn: preface+settings len", v.len = 69);
  Check("conn: preface valid", CheckPreface(v));
  ClearOutput(c);
  v := GetOutput(c);
  Check("conn: cleared", v.len = 0);
  FreeConn(c)
END TestConnLifecycle;

(* ══════════════════════════════════════════════════════════ *)
(* 14. Connection SETTINGS exchange                          *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestConnSettings;
VAR c: H2Conn;
    b: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    s: Settings;
    ok: BOOLEAN;
BEGIN
  InitConn(c);
  ClearOutput(c);

  (* Build a server SETTINGS frame *)
  Init(b, 128);
  Http2Types.InitDefaultSettings(s);
  s.maxFrameSize := 32768;
  BuildSettingsFrame(b, s);
  v := AsView(b);
  (* Parse header + payload *)
  ReadFrameHeader(v, hdr, ok);
  Check("conn.set: hdr ok", ok);
  ReadFramePayload(v, hdr, payload, ok);
  Check("conn.set: payload ok", ok);
  ProcessFrame(c, hdr, payload, ok);
  Check("conn.set: process ok", ok);
  (* Should have generated SETTINGS_ACK *)
  v := GetOutput(c);
  Check("conn.set: ack generated", v.len = 9);
  DecodeHeader(v, hdr, ok);
  Check("conn.set: ack type", hdr.ftype = FrameSettings);
  Check("conn.set: ack flag", (hdr.flags MOD 2) = 1);
  Check("conn.set: remote updated",
        c.remoteSettings.maxFrameSize = 32768);

  Free(b);
  FreeConn(c)
END TestConnSettings;

(* ══════════════════════════════════════════════════════════ *)
(* 15. Connection PING echo                                  *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestConnPing;
VAR c: H2Conn;
    b, pingData: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
    i: CARDINAL;
BEGIN
  InitConn(c);
  ClearOutput(c);

  Init(b, 64);
  Init(pingData, 16);
  i := 0;
  WHILE i < 8 DO
    AppendByte(pingData, 10 + i);
    INC(i)
  END;
  v := AsView(pingData);
  BuildPingFrame(b, v, FALSE);
  v := AsView(b);
  ReadFrameHeader(v, hdr, ok);
  ReadFramePayload(v, hdr, payload, ok);
  ProcessFrame(c, hdr, payload, ok);
  Check("conn.ping: process ok", ok);
  (* Should have PING ACK in output *)
  v := GetOutput(c);
  Check("conn.ping: ack len", v.len = 17);
  DecodeHeader(v, hdr, ok);
  Check("conn.ping: ack type", hdr.ftype = FramePing);
  Check("conn.ping: ack flag", (hdr.flags MOD 2) = 1);
  (* Check echoed data *)
  Check("conn.ping: echo[0]", ViewGetByte(v, 9) = 10);
  Check("conn.ping: echo[7]", ViewGetByte(v, 16) = 17);

  Free(b);
  Free(pingData);
  FreeConn(c)
END TestConnPing;

(* ══════════════════════════════════════════════════════════ *)
(* 16. Connection GOAWAY                                     *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestConnGoaway;
VAR c: H2Conn;
    b: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
BEGIN
  InitConn(c);
  ClearOutput(c);
  Check("conn.goaway: not recv", NOT c.goawayRecv);

  Init(b, 64);
  BuildGoawayFrame(b, 7, ErrNoError);
  v := AsView(b);
  ReadFrameHeader(v, hdr, ok);
  ReadFramePayload(v, hdr, payload, ok);
  ProcessFrame(c, hdr, payload, ok);
  Check("conn.goaway: process ok", ok);
  Check("conn.goaway: recv flag", c.goawayRecv);
  Check("conn.goaway: lastPeer", c.lastPeerStream = 7);
  Check("conn.goaway: code", c.goawayCode = ErrNoError);

  (* No new streams after GOAWAY *)
  Check("conn.goaway: no new stream", OpenStream(c) = 0);

  Free(b);
  FreeConn(c)
END TestConnGoaway;

(* ══════════════════════════════════════════════════════════ *)
(* 17. Connection stream management                          *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestConnStreams;
VAR c: H2Conn;
    sid1, sid2, slot: CARDINAL;
BEGIN
  InitConn(c);
  sid1 := OpenStream(c);
  Check("conn.streams: first=1", sid1 = 1);
  sid2 := OpenStream(c);
  Check("conn.streams: second=3", sid2 = 3);
  Check("conn.streams: numActive", c.numActive = 2);
  slot := FindStream(c, 1);
  Check("conn.streams: find 1", slot < MaxStreams);
  slot := FindStream(c, 99);
  Check("conn.streams: find 99 miss", slot = MaxStreams);
  FreeConn(c)
END TestConnStreams;

(* ══════════════════════════════════════════════════════════ *)
(* 18. Connection flow control                               *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestConnFlowControl;
VAR c: H2Conn;
    ok: BOOLEAN;
BEGIN
  InitConn(c);
  ok := ConsumeConnSendWindow(c, 100);
  Check("conn.flow: consume ok", ok);
  ok := ConsumeConnSendWindow(c, 100000);
  Check("conn.flow: over fail", NOT ok);
  UpdateConnSendWindow(c, 200000);
  ok := ConsumeConnSendWindow(c, 200000);
  Check("conn.flow: after update", ok);
  FreeConn(c)
END TestConnFlowControl;

(* ══════════════════════════════════════════════════════════ *)
(* 19. WINDOW_UPDATE via connection                          *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestConnWindowUpdate;
VAR c: H2Conn;
    b: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
    sid: CARDINAL;
BEGIN
  InitConn(c);
  ClearOutput(c);

  Init(b, 64);
  (* Connection-level WINDOW_UPDATE *)
  BuildWindowUpdateFrame(b, 0, 1000);
  v := AsView(b);
  ReadFrameHeader(v, hdr, ok);
  ReadFramePayload(v, hdr, payload, ok);
  ProcessFrame(c, hdr, payload, ok);
  Check("conn.winup: process ok", ok);
  Check("conn.winup: window increased",
        c.connSendWindow = VAL(INTEGER, DefaultInitialWindowSize) + 1000);

  Free(b);
  FreeConn(c)
END TestConnWindowUpdate;

(* ══════════════════════════════════════════════════════════ *)
(* 20. DATA frame header                                     *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestDataHeader;
VAR b: Buf;
    v: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
BEGIN
  Init(b, 64);
  EncodeDataHeader(b, 7, 256, TRUE);
  Check("data.hdr: len=9", b.len = 9);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("data.hdr: type", hdr.ftype = FrameData);
  Check("data.hdr: streamId", hdr.streamId = 7);
  Check("data.hdr: payload len", hdr.length = 256);
  Check("data.hdr: end_stream", (hdr.flags MOD 2) = 1);

  Clear(b);
  EncodeDataHeader(b, 9, 100, FALSE);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("data.hdr: no end_stream", (hdr.flags MOD 2) = 0);
  Free(b)
END TestDataHeader;

(* ══════════════════════════════════════════════════════════ *)
(* 21. HEADERS frame header                                  *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestHeadersHeader;
VAR b: Buf;
    v: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
BEGIN
  Init(b, 64);
  EncodeHeadersHeader(b, 1, 50, TRUE, TRUE);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("hdrs.hdr: type", hdr.ftype = FrameHeaders);
  Check("hdrs.hdr: streamId", hdr.streamId = 1);
  Check("hdrs.hdr: payload len", hdr.length = 50);
  Check("hdrs.hdr: end_stream", (hdr.flags MOD 2) = 1);
  Check("hdrs.hdr: end_headers", (hdr.flags DIV 4) MOD 2 = 1);

  Clear(b);
  EncodeHeadersHeader(b, 3, 100, FALSE, TRUE);
  v := AsView(b);
  DecodeHeader(v, hdr, ok);
  Check("hdrs.hdr: no es", (hdr.flags MOD 2) = 0);
  Check("hdrs.hdr: eh", (hdr.flags DIV 4) MOD 2 = 1);
  Free(b)
END TestHeadersHeader;

(* ══════════════════════════════════════════════════════════ *)
(* 22. HPACK encode/decode roundtrip                         *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestHpackRoundtrip;
VAR encBuf: Buf;
    dt1, dt2: DynTable;
    hdrs: ARRAY [0..3] OF HeaderEntry;
    outHdrs: ARRAY [0..3] OF HeaderEntry;
    v: BytesView;
    numOut: CARDINAL;
    ok: BOOLEAN;
    i: CARDINAL;
BEGIN
  Init(encBuf, 512);
  DynInit(dt1, 4096);
  DynInit(dt2, 4096);

  (* Set up headers: :method GET, :path /, :scheme https, custom *)
  hdrs[0].name[0] := ":"; hdrs[0].name[1] := "m";
  hdrs[0].name[2] := "e"; hdrs[0].name[3] := "t";
  hdrs[0].name[4] := "h"; hdrs[0].name[5] := "o";
  hdrs[0].name[6] := "d"; hdrs[0].nameLen := 7;
  hdrs[0].value[0] := "G"; hdrs[0].value[1] := "E";
  hdrs[0].value[2] := "T"; hdrs[0].valLen := 3;

  hdrs[1].name[0] := ":"; hdrs[1].name[1] := "p";
  hdrs[1].name[2] := "a"; hdrs[1].name[3] := "t";
  hdrs[1].name[4] := "h"; hdrs[1].nameLen := 5;
  hdrs[1].value[0] := "/"; hdrs[1].valLen := 1;

  hdrs[2].name[0] := ":"; hdrs[2].name[1] := "s";
  hdrs[2].name[2] := "c"; hdrs[2].name[3] := "h";
  hdrs[2].name[4] := "e"; hdrs[2].name[5] := "m";
  hdrs[2].name[6] := "e"; hdrs[2].nameLen := 7;
  hdrs[2].value[0] := "h"; hdrs[2].value[1] := "t";
  hdrs[2].value[2] := "t"; hdrs[2].value[3] := "p";
  hdrs[2].value[4] := "s"; hdrs[2].valLen := 5;

  hdrs[3].name[0] := "x"; hdrs[3].name[1] := "-";
  hdrs[3].name[2] := "i"; hdrs[3].name[3] := "d";
  hdrs[3].nameLen := 4;
  hdrs[3].value[0] := "4"; hdrs[3].value[1] := "2";
  hdrs[3].valLen := 2;

  EncodeHeaderBlock(encBuf, dt1, hdrs, 4);
  Check("hpack.rt: encoded len > 0", encBuf.len > 0);

  v := AsView(encBuf);
  DecodeHeaderBlock(v, dt2, outHdrs, 4, numOut, ok);
  Check("hpack.rt: decode ok", ok);
  Check("hpack.rt: numOut=4", numOut = 4);

  (* Verify :method GET *)
  Check("hpack.rt: h0 name", (outHdrs[0].nameLen = 7) AND
                               (outHdrs[0].name[1] = "m"));
  Check("hpack.rt: h0 val", (outHdrs[0].valLen = 3) AND
                              (outHdrs[0].value[0] = "G"));

  (* Verify :path / *)
  Check("hpack.rt: h1 name", (outHdrs[1].nameLen = 5) AND
                               (outHdrs[1].name[1] = "p"));
  Check("hpack.rt: h1 val", (outHdrs[1].valLen = 1) AND
                              (outHdrs[1].value[0] = "/"));

  (* Verify custom header in dynamic table *)
  Check("hpack.rt: h3 name", (outHdrs[3].nameLen = 4) AND
                               (outHdrs[3].name[0] = "x"));
  Check("hpack.rt: h3 val", (outHdrs[3].valLen = 2) AND
                              (outHdrs[3].value[0] = "4"));

  (* Custom header should be in dt2 *)
  Check("hpack.rt: dt2 count", DynCount(dt2) >= 1);

  Free(encBuf)
END TestHpackRoundtrip;

(* ══════════════════════════════════════════════════════════ *)
(* 23. Test utility: frame builder/reader                    *)
(* ══════════════════════════════════════════════════════════ *)

PROCEDURE TestUtil;
VAR b, payBuf: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    ok: BOOLEAN;
BEGIN
  Init(b, 64);
  Init(payBuf, 16);
  AppendByte(payBuf, 0AAH);
  AppendByte(payBuf, 0BBH);
  v := AsView(payBuf);
  BuildFrame(b, FrameData, FlagEndStream, 11, v);
  Check("util: frame len", b.len = 9 + 2);
  v := AsView(b);
  ReadFrameHeader(v, hdr, ok);
  Check("util: read hdr ok", ok);
  Check("util: type", hdr.ftype = FrameData);
  Check("util: streamId", hdr.streamId = 11);
  Check("util: payload len", hdr.length = 2);
  ReadFramePayload(v, hdr, payload, ok);
  Check("util: payload ok", ok);
  Check("util: payload[0]", ViewGetByte(payload, 0) = 0AAH);
  Check("util: payload[1]", ViewGetByte(payload, 1) = 0BBH);

  Free(b);
  Free(payBuf)
END TestUtil;

(* ══════════════════════════════════════════════════════════ *)

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestFrameHeader;
  TestPreface;
  TestSettings;
  TestPing;
  TestGoaway;
  TestWindowUpdate;
  TestRstStream;
  TestHpackInt;
  TestStaticTable;
  TestDynTable;
  TestStreamFsm;
  TestStreamFlowControl;
  TestConnLifecycle;
  TestConnSettings;
  TestConnPing;
  TestConnGoaway;
  TestConnStreams;
  TestConnFlowControl;
  TestConnWindowUpdate;
  TestDataHeader;
  TestHeadersHeader;
  TestHpackRoundtrip;
  TestUtil;

  WriteLn;
  WriteString("HTTP/2 tests: ");
  WriteCard(passed, 0);
  WriteString(" passed, ");
  WriteCard(failed, 0);
  WriteString(" failed, ");
  WriteCard(total, 0);
  WriteString(" total");
  WriteLn
END h2_tests.
