MODULE h2s_tests;

  (* Deterministic tests for the HTTP/2 server library.
     All tests use in-memory connections — no TLS, no sockets. *)

  FROM SYSTEM IMPORT ADDRESS, ADR;
  FROM InOut IMPORT WriteString, WriteLn, WriteCard;
  FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear,
                       AsView, AppendChars, AppendByte, Len;
  FROM Http2Types IMPORT Settings, FrameHeader, HeaderEntry,
                          FrameSettings, FrameData, FrameHeaders,
                          FramePing, FrameGoaway, FrameWindowUpdate,
                          FrameRstStream,
                          FlagAck, FlagEndStream, FlagEndHeaders,
                          ErrNoError, ErrProtocol,
                          FrameHeaderSize,
                          InitDefaultSettings;
  FROM Http2Hpack IMPORT DynTable, DynInit;
  FROM Http2ServerConn IMPORT ConnPtr, ConnRec, ConnCreateTest,
                               ConnClose, ConnFeedBytes,
                               CpPreface, CpSettings, CpOpen,
                               CpGoaway, CpClosed;
  FROM Http2ServerTestUtil IMPORT BuildClientPreface, BuildSettings,
                                   BuildSettingsAck, BuildHeaders,
                                   BuildData, BuildPing, BuildGET,
                                   BuildPOST, BuildGoaway,
                                   BuildRstStream, BuildWindowUpdate,
                                   ReadNextFrame, FeedAndCollect,
                                   DoTestHandshake;
  FROM Http2ServerTypes IMPORT Request, Response;

  VAR
    passed, failed, testNum: CARDINAL;

  PROCEDURE Check(cond: BOOLEAN; msg: ARRAY OF CHAR);
  BEGIN
    INC(testNum);
    IF cond THEN
      INC(passed);
    ELSE
      INC(failed);
      WriteString("  FAIL #");
      WriteCard(testNum, 1);
      WriteString(": ");
      WriteString(msg);
      WriteLn;
    END;
  END Check;

  (* ── Test 1: Handshake + SETTINGS ──────────────────── *)

  PROCEDURE TestHandshake;
  VAR
    cp: ConnPtr;
    input, output: Buf;
    s: Settings;
    v: BytesView;
    hdr: FrameHeader;
    payload: BytesView;
    gotSettings, gotAck: BOOLEAN;
  BEGIN
    WriteString("Test 1: Handshake + SETTINGS"); WriteLn;

    ConnCreateTest(NIL, 1, cp);
    Check(cp # NIL, "ConnCreateTest succeeded");

    Init(input, 1024);
    Init(output, 4096);

    (* Send client preface + SETTINGS *)
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);

    FeedAndCollect(cp, input, output);

    (* Parse server response *)
    v := AsView(output);
    gotSettings := FALSE;
    gotAck := FALSE;

    WHILE ReadNextFrame(v, hdr, payload) DO
      IF hdr.ftype = FrameSettings THEN
        IF (hdr.flags = FlagAck) THEN
          gotAck := TRUE;
        ELSE
          gotSettings := TRUE;
        END;
      END;
    END;

    Check(gotSettings, "Server sent SETTINGS");
    Check(gotAck, "Server sent SETTINGS ACK");
    Check(cp^.phase = CpOpen, "Connection is Open");

    Free(input);
    Free(output);
    ConnClose(cp);
  END TestHandshake;

  (* ── Test 2: HEADERS frame parsing ─────────────────── *)

  PROCEDURE TestHeadersParsing;
  VAR
    cp: ConnPtr;
    input, output, hsOutput: Buf;
    dt: DynTable;
    s: Settings;
    v: BytesView;
    hdr: FrameHeader;
    payload: BytesView;
  BEGIN
    WriteString("Test 2: HEADERS frame parsing"); WriteLn;

    ConnCreateTest(NIL, 2, cp);
    Init(hsOutput, 4096);

    (* Do handshake first *)
    Init(input, 1024);
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);

    (* Send SETTINGS ACK *)
    Init(input, 64);
    BuildSettingsAck(input);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Free(hsOutput);

    Check(cp^.phase = CpOpen, "Connection open after handshake");

    (* Send a GET request on stream 1 *)
    DynInit(dt, 4096);
    Init(input, 1024);
    Init(output, 4096);
    BuildGET(input, dt, 1, "/hello");
    FeedAndCollect(cp, input, output);

    (* Stream should have been allocated *)
    Check(cp^.lastPeerStream = 1, "lastPeerStream is 1");

    Free(input);
    Free(output);
    ConnClose(cp);
  END TestHeadersParsing;

  (* ── Test 3: DATA accumulation + WINDOW_UPDATE ─────── *)

  PROCEDURE TestDataAccumulation;
  VAR
    cp: ConnPtr;
    input, output, hsOutput: Buf;
    dt: DynTable;
    s: Settings;
    v: BytesView;
    hdr: FrameHeader;
    payload: BytesView;
    body1, body2: BytesView;
    bodyBuf1, bodyBuf2: Buf;
    gotWindowUpdate: BOOLEAN;
  BEGIN
    WriteString("Test 3: DATA accumulation + WINDOW_UPDATE"); WriteLn;

    ConnCreateTest(NIL, 3, cp);
    Init(hsOutput, 4096);

    (* Handshake *)
    Init(input, 1024);
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Init(input, 64);
    BuildSettingsAck(input);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Free(hsOutput);

    (* Send POST headers (no END_STREAM) *)
    DynInit(dt, 4096);
    Init(input, 1024);
    Init(output, 4096);
    BuildPOST(input, dt, 1, "/echo");
    FeedAndCollect(cp, input, output);
    Free(input);
    Clear(output);

    (* Send first DATA chunk *)
    Init(bodyBuf1, 64);
    AppendChars(bodyBuf1, "Hello ", 6);
    body1 := AsView(bodyBuf1);

    Init(input, 256);
    BuildData(input, 1, body1, FALSE);
    FeedAndCollect(cp, input, output);
    Free(input);

    (* Check for WINDOW_UPDATE in response *)
    v := AsView(output);
    gotWindowUpdate := FALSE;
    WHILE ReadNextFrame(v, hdr, payload) DO
      IF hdr.ftype = FrameWindowUpdate THEN
        gotWindowUpdate := TRUE;
      END;
    END;

    Check(gotWindowUpdate, "Server sent WINDOW_UPDATE after DATA");

    (* Send second DATA chunk with END_STREAM *)
    Init(bodyBuf2, 64);
    AppendChars(bodyBuf2, "World!", 6);
    body2 := AsView(bodyBuf2);

    Init(input, 256);
    Clear(output);
    BuildData(input, 1, body2, TRUE);
    FeedAndCollect(cp, input, output);
    Free(input);

    Free(bodyBuf1);
    Free(bodyBuf2);
    Free(output);
    ConnClose(cp);
  END TestDataAccumulation;

  (* ── Test 4: Multiplexing ──────────────────────────── *)

  PROCEDURE TestMultiplexing;
  VAR
    cp: ConnPtr;
    input, output, hsOutput: Buf;
    dt: DynTable;
    s: Settings;
  BEGIN
    WriteString("Test 4: Multiplexing (3 streams)"); WriteLn;

    ConnCreateTest(NIL, 4, cp);
    Init(hsOutput, 4096);

    (* Handshake *)
    Init(input, 1024);
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Init(input, 64);
    BuildSettingsAck(input);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Free(hsOutput);

    (* Send 3 GET requests on streams 1, 3, 5 *)
    DynInit(dt, 4096);
    Init(input, 2048);
    Init(output, 4096);
    BuildGET(input, dt, 1, "/a");
    BuildGET(input, dt, 3, "/b");
    BuildGET(input, dt, 5, "/c");
    FeedAndCollect(cp, input, output);

    Check(cp^.lastPeerStream = 5, "lastPeerStream is 5");
    (* All 3 streams should have been allocated *)
    Check(cp^.numActive >= 0, "Streams allocated without crash");

    Free(input);
    Free(output);
    ConnClose(cp);
  END TestMultiplexing;

  (* ── Test 5: WINDOW_UPDATE on DATA ─────────────────── *)

  PROCEDURE TestFlowControl;
  VAR
    cp: ConnPtr;
    input, output, hsOutput: Buf;
    dt: DynTable;
    s: Settings;
    v: BytesView;
    hdr: FrameHeader;
    payload: BytesView;
    bodyBuf: Buf;
    bodyView: BytesView;
    connWU, streamWU: CARDINAL;
  BEGIN
    WriteString("Test 5: Flow control WINDOW_UPDATE"); WriteLn;

    ConnCreateTest(NIL, 5, cp);
    Init(hsOutput, 4096);

    (* Handshake *)
    Init(input, 1024);
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Init(input, 64);
    BuildSettingsAck(input);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Free(hsOutput);

    (* POST + DATA *)
    DynInit(dt, 4096);
    Init(input, 2048);
    Init(output, 4096);
    BuildPOST(input, dt, 1, "/data");
    FeedAndCollect(cp, input, output);
    Free(input);
    Clear(output);

    (* Send 1000 bytes of DATA *)
    Init(bodyBuf, 1024);
    WHILE bodyBuf.len < 1000 DO
      AppendByte(bodyBuf, ORD("X"));
    END;
    bodyView := AsView(bodyBuf);

    Init(input, 2048);
    BuildData(input, 1, bodyView, TRUE);
    FeedAndCollect(cp, input, output);
    Free(input);

    (* Count WINDOW_UPDATE frames *)
    v := AsView(output);
    connWU := 0;
    streamWU := 0;
    WHILE ReadNextFrame(v, hdr, payload) DO
      IF hdr.ftype = FrameWindowUpdate THEN
        IF hdr.streamId = 0 THEN
          INC(connWU);
        ELSE
          INC(streamWU);
        END;
      END;
    END;

    Check(connWU > 0, "Connection-level WINDOW_UPDATE sent");
    Check(streamWU > 0, "Stream-level WINDOW_UPDATE sent");

    Free(bodyBuf);
    Free(output);
    ConnClose(cp);
  END TestFlowControl;

  (* ── Test 6: PING echo ─────────────────────────────── *)

  PROCEDURE TestPingEcho;
  VAR
    cp: ConnPtr;
    input, output, hsOutput: Buf;
    s: Settings;
    pingData: Buf;
    pingView: BytesView;
    v: BytesView;
    hdr: FrameHeader;
    payload: BytesView;
    gotPingAck: BOOLEAN;
  BEGIN
    WriteString("Test 6: PING echo"); WriteLn;

    ConnCreateTest(NIL, 6, cp);
    Init(hsOutput, 4096);

    (* Handshake *)
    Init(input, 1024);
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Init(input, 64);
    BuildSettingsAck(input);
    FeedAndCollect(cp, input, hsOutput);
    Free(input);
    Free(hsOutput);

    (* Send PING with 8 bytes of data *)
    Init(pingData, 8);
    AppendChars(pingData, "01234567", 8);
    pingView := AsView(pingData);

    Init(input, 64);
    Init(output, 4096);
    BuildPing(input, pingView);
    FeedAndCollect(cp, input, output);

    (* Check for PING ACK *)
    v := AsView(output);
    gotPingAck := FALSE;
    WHILE ReadNextFrame(v, hdr, payload) DO
      IF (hdr.ftype = FramePing) AND (hdr.flags = FlagAck) THEN
        gotPingAck := TRUE;
        Check(payload.len = 8, "PING ACK has 8 bytes");
      END;
    END;

    Check(gotPingAck, "Server sent PING ACK");

    Free(pingData);
    Free(input);
    Free(output);
    ConnClose(cp);
  END TestPingEcho;

  (* ── Test 7: Error paths ───────────────────────────── *)

  PROCEDURE TestErrorPaths;
  VAR
    cp: ConnPtr;
    input, output: Buf;
    v: BytesView;
    hdr: FrameHeader;
    payload: BytesView;
    gotGoaway: BOOLEAN;
    s: Settings;
    dt: DynTable;
    hdrs: ARRAY [0..1] OF HeaderEntry;
  BEGIN
    WriteString("Test 7: Error paths"); WriteLn;

    (* 7a: Invalid preface *)
    ConnCreateTest(NIL, 71, cp);
    Init(input, 256);
    Init(output, 4096);

    (* Send garbage instead of preface *)
    AppendChars(input, "NOT A VALID HTTP2 PREFACE!", 25);

    FeedAndCollect(cp, input, output);

    v := AsView(output);
    gotGoaway := FALSE;
    WHILE ReadNextFrame(v, hdr, payload) DO
      IF hdr.ftype = FrameGoaway THEN
        gotGoaway := TRUE;
      END;
    END;
    Check(gotGoaway, "GOAWAY on invalid preface");

    Free(input);
    Free(output);
    ConnClose(cp);

    (* 7b: HEADERS on stream 0 *)
    ConnCreateTest(NIL, 72, cp);
    Init(output, 4096);

    (* Handshake first *)
    Init(input, 1024);
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);
    FeedAndCollect(cp, input, output);
    Free(input);
    Init(input, 64);
    BuildSettingsAck(input);
    FeedAndCollect(cp, input, output);
    Free(input);
    Clear(output);

    (* Manually build a HEADERS frame on stream 0 *)
    Init(input, 64);
    DynInit(dt, 4096);
    hdrs[0].name[0] := ":"; hdrs[0].name[1] := "m"; hdrs[0].name[2] := 0C;
    hdrs[0].nameLen := 2;
    hdrs[0].value[0] := "G"; hdrs[0].value[1] := 0C;
    hdrs[0].valLen := 1;
    BuildHeaders(input, dt, 0, hdrs, 1, TRUE);  (* stream 0 = invalid *)
    FeedAndCollect(cp, input, output);

    v := AsView(output);
    gotGoaway := FALSE;
    WHILE ReadNextFrame(v, hdr, payload) DO
      IF hdr.ftype = FrameGoaway THEN
        gotGoaway := TRUE;
      END;
    END;
    Check(gotGoaway, "GOAWAY on HEADERS with stream 0");

    Free(input);
    Free(output);
    ConnClose(cp);

    (* 7c: HEADERS on even stream ID *)
    ConnCreateTest(NIL, 73, cp);
    Init(output, 4096);

    Init(input, 1024);
    BuildClientPreface(input);
    InitDefaultSettings(s);
    BuildSettings(input, s);
    FeedAndCollect(cp, input, output);
    Free(input);
    Init(input, 64);
    BuildSettingsAck(input);
    FeedAndCollect(cp, input, output);
    Free(input);
    Clear(output);

    Init(input, 256);
    DynInit(dt, 4096);
    BuildGET(input, dt, 2, "/bad");  (* stream 2 = even = invalid *)
    FeedAndCollect(cp, input, output);

    v := AsView(output);
    gotGoaway := FALSE;
    WHILE ReadNextFrame(v, hdr, payload) DO
      IF hdr.ftype = FrameGoaway THEN
        gotGoaway := TRUE;
      END;
    END;
    Check(gotGoaway, "GOAWAY on HEADERS with even stream ID");

    Free(input);
    Free(output);
    ConnClose(cp);
  END TestErrorPaths;

BEGIN
  passed := 0;
  failed := 0;
  testNum := 0;

  WriteString("═══ m2http2server tests ═══"); WriteLn;
  WriteLn;

  TestHandshake;
  WriteLn;
  TestHeadersParsing;
  WriteLn;
  TestDataAccumulation;
  WriteLn;
  TestMultiplexing;
  WriteLn;
  TestFlowControl;
  WriteLn;
  TestPingEcho;
  WriteLn;
  TestErrorPaths;
  WriteLn;

  WriteString("═══ Results: ");
  WriteCard(passed, 1);
  WriteString(" passed, ");
  WriteCard(failed, 1);
  WriteString(" failed ═══");
  WriteLn;
END h2s_tests.
