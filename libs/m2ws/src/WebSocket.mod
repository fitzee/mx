IMPLEMENTATION MODULE WebSocket;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE, BYTE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Promise, Value, Error, Result,
                    PromiseCreate, Resolve, Reject,
                    GetResultIfSettled;
IMPORT Promise;
FROM Poller IMPORT EvRead, EvWrite;
IMPORT EventLoop;
FROM URI IMPORT URIRec;
IMPORT URI;
IMPORT DNS;
FROM DNS IMPORT AddrRec, AddrPtr;
FROM Sockets IMPORT Socket, InvalidSocket, AF_INET, SOCK_STREAM,
                    SocketCreate, CloseSocket;
IMPORT Sockets;
FROM DnsBridge IMPORT m2_connect_ipv4, m2_getsockopt_error;
IMPORT TLS;
IMPORT Stream;
FROM WsFrame IMPORT Opcode, FrameHeader, MaxFrameHeader,
                    DecodeHeader, EncodeHeader, ApplyMask,
                    GenerateMask, OpcodeToInt,
                    OpText, OpBinary, OpClose, OpPing, OpPong,
                    OpContinuation;
IMPORT WsFrame;
FROM WsBridge IMPORT m2_ws_sha1, m2_ws_base64_encode;

(* ── Internal constants ──────────────────────────────────── *)

CONST
  StConnecting  = 0;
  StSending     = 1;
  StRecvUpgrade = 2;
  StOpen        = 3;
  StClosing     = 4;
  StClosed      = 5;
  StError       = 6;
  StHandshaking = 7;

  RecvBufSize = 8192;
  SendBufSize = 4096;
  MaxReqSize  = 2048;
  LineBufSize = 1024;
  MaxMsgSize  = 1048576;  (* 1 MB max message *)

  WsKeyLen    = 24;  (* base64-encoded 16-byte nonce *)
  AcceptLen   = 28;  (* base64-encoded SHA-1 hash *)

(* ── Internal types ──────────────────────────────────────── *)

TYPE
  CharPtr = POINTER TO CHAR;

  WsRec = RECORD
    state       : INTEGER;
    sock        : Socket;
    promise     : Promise;     (* connect/close promise *)
    loop        : EventLoop.Loop;
    sched       : Scheduler;
    (* Transport *)
    useTLS      : BOOLEAN;
    tlsCtx      : TLS.TLSContext;
    tlsSess     : TLS.TLSSession;
    stream      : Stream.Stream;
    (* Handshake *)
    request     : ARRAY [0..MaxReqSize-1] OF CHAR;
    reqLen      : INTEGER;
    reqSent     : INTEGER;
    wsKey       : ARRAY [0..WsKeyLen-1] OF CHAR;
    expectedAccept: ARRAY [0..AcceptLen] OF CHAR;
    expectedAcceptLen: INTEGER;
    (* Receive buffer *)
    recvBuf     : ARRAY [0..RecvBufSize-1] OF CHAR;
    recvLen     : INTEGER;
    recvPos     : INTEGER;
    (* Upgrade response parsing *)
    lineBuf     : ARRAY [0..LineBufSize-1] OF CHAR;
    lineLen     : INTEGER;
    gotStatus   : BOOLEAN;
    gotUpgrade  : BOOLEAN;
    gotAccept   : BOOLEAN;
    statusOk    : BOOLEAN;
    (* Message assembly *)
    msgBuf      : ADDRESS;     (* heap buffer for reassembly *)
    msgLen      : CARDINAL;
    msgCap      : CARDINAL;
    msgOpcode   : Opcode;
    (* Callback *)
    handler     : MessageProc;
    handlerCtx  : ADDRESS;
    (* Close *)
    closePromise: Promise;
  END;

  WsPtr = POINTER TO WsRec;

(* ── String helpers ──────────────────────────────────────── *)

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): INTEGER;
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
  RETURN i
END StrLen;

PROCEDURE AppendCh(w: WsPtr; ch: CHAR);
BEGIN
  IF w^.reqLen < MaxReqSize THEN
    w^.request[w^.reqLen] := ch;
    INC(w^.reqLen)
  END
END AppendCh;

PROCEDURE AppendStr(w: WsPtr; VAR s: ARRAY OF CHAR);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    AppendCh(w, s[i]);
    INC(i)
  END
END AppendStr;

PROCEDURE AppendCRLF(w: WsPtr);
BEGIN
  AppendCh(w, CHR(13));
  AppendCh(w, CHR(10))
END AppendCRLF;

PROCEDURE ToLower(ch: CHAR): CHAR;
BEGIN
  IF (ch >= 'A') AND (ch <= 'Z') THEN
    RETURN CHR(ORD(ch) + 32)
  END;
  RETURN ch
END ToLower;

PROCEDURE StrEqCI(VAR a, b: ARRAY OF CHAR; aLen, bLen: INTEGER): BOOLEAN;
VAR i: INTEGER;
BEGIN
  IF aLen # bLen THEN RETURN FALSE END;
  FOR i := 0 TO aLen - 1 DO
    IF ToLower(a[i]) # ToLower(b[i]) THEN RETURN FALSE END
  END;
  RETURN TRUE
END StrEqCI;

(* ── Pointer helpers ─────────────────────────────────────── *)

PROCEDURE OffsetPtr(base: ADDRESS; n: INTEGER): ADDRESS;
BEGIN
  RETURN VAL(ADDRESS, LONGCARD(base) + LONGCARD(n))
END OffsetPtr;

PROCEDURE CopyBytes(src, dst: ADDRESS; len: INTEGER);
VAR sp, dp: CharPtr; i: INTEGER;
BEGIN
  FOR i := 0 TO len - 1 DO
    sp := CharPtr(LONGCARD(src) + LONGCARD(i));
    dp := CharPtr(LONGCARD(dst) + LONGCARD(i));
    dp^ := sp^
  END
END CopyBytes;

(* ── WebSocket key generation ────────────────────────────── *)

(* RFC 6455 requires a base64-encoded 16-byte random value.
   We generate it using the mask PRNG and base64 encode it. *)

PROCEDURE GenerateKey(w: WsPtr);
VAR
  raw: ARRAY [0..15] OF CHAR;
  outLen: INTEGER;
  i: INTEGER;
  mask: ARRAY [0..3] OF CHAR;
BEGIN
  (* Generate 16 random bytes using 4 mask generations *)
  FOR i := 0 TO 3 DO
    GenerateMask(mask);
    raw[i*4]   := mask[0];
    raw[i*4+1] := mask[1];
    raw[i*4+2] := mask[2];
    raw[i*4+3] := mask[3]
  END;
  m2_ws_base64_encode(ADR(raw), 16, ADR(w^.wsKey), WsKeyLen, outLen)
END GenerateKey;

(* Compute expected Sec-WebSocket-Accept value.
   Accept = Base64(SHA1(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11")) *)

PROCEDURE ComputeExpectedAccept(w: WsPtr);
VAR
  concat: ARRAY [0..79] OF CHAR;
  concatLen: INTEGER;
  sha1Out: ARRAY [0..19] OF CHAR;
  guid: ARRAY [0..35] OF CHAR;
  i: INTEGER;
BEGIN
  guid := "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
  concatLen := 0;
  (* Copy key *)
  FOR i := 0 TO WsKeyLen - 1 DO
    concat[concatLen] := w^.wsKey[i];
    INC(concatLen)
  END;
  (* Append GUID *)
  i := 0;
  WHILE (i <= HIGH(guid)) AND (guid[i] # 0C) DO
    concat[concatLen] := guid[i];
    INC(concatLen);
    INC(i)
  END;

  m2_ws_sha1(ADR(concat), concatLen, ADR(sha1Out));
  m2_ws_base64_encode(ADR(sha1Out), 20,
                      ADR(w^.expectedAccept), AcceptLen + 1,
                      w^.expectedAcceptLen)
END ComputeExpectedAccept;

(* ── Build upgrade request ───────────────────────────────── *)

PROCEDURE BuildUpgradeRequest(w: WsPtr; VAR uri: URIRec);
VAR
  rpath: ARRAY [0..2047] OF CHAR;
  rpLen: INTEGER;
  ust: URI.Status;
BEGIN
  w^.reqLen := 0;
  AppendStr(w, "GET ");

  ust := URI.RequestPath(uri, rpath, rpLen);
  IF rpLen > 0 THEN
    AppendStr(w, rpath)
  ELSE
    AppendCh(w, '/')
  END;

  AppendStr(w, " HTTP/1.1");
  AppendCRLF(w);
  AppendStr(w, "Host: ");
  AppendStr(w, uri.host);
  AppendCRLF(w);
  AppendStr(w, "Upgrade: websocket");
  AppendCRLF(w);
  AppendStr(w, "Connection: Upgrade");
  AppendCRLF(w);
  AppendStr(w, "Sec-WebSocket-Key: ");
  AppendStr(w, w^.wsKey);
  AppendCRLF(w);
  AppendStr(w, "Sec-WebSocket-Version: 13");
  AppendCRLF(w);
  AppendCRLF(w)
END BuildUpgradeRequest;

(* ── TLS-aware I/O helpers ───────────────────────────────── *)

PROCEDURE DoSend(w: WsPtr; buf: ADDRESS; len: INTEGER): INTEGER;
VAR n: INTEGER; st: Stream.Status;
BEGIN
  st := Stream.TryWrite(w^.stream, buf, len, n);
  IF st = Stream.OK THEN RETURN n
  ELSIF st = Stream.WouldBlock THEN RETURN -2
  ELSE RETURN -1
  END
END DoSend;

PROCEDURE DoRecv(w: WsPtr; buf: ADDRESS; max: INTEGER): INTEGER;
VAR n: INTEGER; st: Stream.Status;
BEGIN
  st := Stream.TryRead(w^.stream, buf, max, n);
  IF st = Stream.OK THEN RETURN n
  ELSIF st = Stream.StreamClosed THEN RETURN 0
  ELSIF st = Stream.WouldBlock THEN RETURN -2
  ELSE RETURN -1
  END
END DoRecv;

(* ── Cleanup ─────────────────────────────────────────────── *)

PROCEDURE CleanupWs(w: WsPtr);
VAR est: EventLoop.Status; sst: Sockets.Status;
    tst: TLS.Status; stst: Stream.Status;
BEGIN
  IF w^.stream # NIL THEN
    stst := Stream.Destroy(w^.stream);
    w^.stream := NIL
  END;
  IF w^.useTLS THEN
    IF w^.tlsSess # NIL THEN
      tst := TLS.Shutdown(w^.tlsSess);
      tst := TLS.SessionDestroy(w^.tlsSess);
      w^.tlsSess := NIL
    END;
    IF w^.tlsCtx # NIL THEN
      tst := TLS.ContextDestroy(w^.tlsCtx);
      w^.tlsCtx := NIL
    END
  END;
  IF w^.sock # InvalidSocket THEN
    est := EventLoop.UnwatchFd(w^.loop, w^.sock);
    sst := Sockets.CloseSocket(w^.sock);
    w^.sock := InvalidSocket
  END;
  IF w^.msgBuf # NIL THEN
    DEALLOCATE(w^.msgBuf, w^.msgCap);
    w^.msgBuf := NIL;
    w^.msgLen := 0;
    w^.msgCap := 0
  END
END CleanupWs;

PROCEDURE FailWs(w: WsPtr; code: INTEGER);
VAR e: Error; dummy: Promise.Status;
BEGIN
  CleanupWs(w);
  e.code := code;
  e.ptr := NIL;
  IF w^.promise # NIL THEN
    dummy := Reject(w^.promise, e);
    w^.promise := NIL
  END;
  IF w^.closePromise # NIL THEN
    dummy := Reject(w^.closePromise, e);
    w^.closePromise := NIL
  END;
  w^.state := StError
END FailWs;

(* ── Message buffer management ───────────────────────────── *)

PROCEDURE EnsureMsgCap(w: WsPtr; needed: CARDINAL): BOOLEAN;
VAR newCap: CARDINAL; newBuf, oldBuf: ADDRESS;
BEGIN
  IF needed <= w^.msgCap THEN RETURN TRUE END;
  IF needed > MaxMsgSize THEN RETURN FALSE END;

  newCap := w^.msgCap * 2;
  IF newCap < needed THEN newCap := needed END;
  IF newCap < 256 THEN newCap := 256 END;
  IF newCap > MaxMsgSize THEN newCap := MaxMsgSize END;

  ALLOCATE(newBuf, newCap);
  IF newBuf = NIL THEN RETURN FALSE END;

  IF (w^.msgBuf # NIL) AND (w^.msgLen > 0) THEN
    CopyBytes(w^.msgBuf, newBuf, VAL(INTEGER, w^.msgLen))
  END;

  IF w^.msgBuf # NIL THEN
    oldBuf := w^.msgBuf;
    DEALLOCATE(oldBuf, w^.msgCap)
  END;

  w^.msgBuf := newBuf;
  w^.msgCap := newCap;
  RETURN TRUE
END EnsureMsgCap;

(* ── Send a raw frame ────────────────────────────────────── *)

PROCEDURE SendFrame(w: WsPtr; op: Opcode; fin: BOOLEAN;
                    data: ADDRESS; len: CARDINAL): INTEGER;
VAR
  hdr: FrameHeader;
  hdrBuf: ARRAY [0..MaxFrameHeader-1] OF CHAR;
  hdrLen: CARDINAL;
  sent, n: INTEGER;
  sendBuf: ADDRESS;
  sendLen: CARDINAL;
  mask: ARRAY [0..3] OF CHAR;
  i: CARDINAL;
BEGIN
  hdr.fin := fin;
  hdr.opcode := op;
  hdr.masked := TRUE;
  hdr.payloadLen := len;
  GenerateMask(mask);
  hdr.maskKey[0] := mask[0];
  hdr.maskKey[1] := mask[1];
  hdr.maskKey[2] := mask[2];
  hdr.maskKey[3] := mask[3];

  hdrLen := EncodeHeader(hdr, ADR(hdrBuf), MaxFrameHeader);
  IF hdrLen = 0 THEN RETURN -1 END;

  (* Send header *)
  sent := 0;
  WHILE sent < VAL(INTEGER, hdrLen) DO
    n := DoSend(w, OffsetPtr(ADR(hdrBuf), sent),
                VAL(INTEGER, hdrLen) - sent);
    IF n > 0 THEN
      sent := sent + n
    ELSIF n = -2 THEN
      (* would block -- not handling async send queueing here *)
      RETURN -2
    ELSE
      RETURN -1
    END
  END;

  (* Send masked payload *)
  IF len > 0 THEN
    ALLOCATE(sendBuf, len);
    IF sendBuf = NIL THEN RETURN -1 END;
    CopyBytes(data, sendBuf, VAL(INTEGER, len));
    ApplyMask(sendBuf, len, mask, 0);

    sent := 0;
    WHILE sent < VAL(INTEGER, len) DO
      n := DoSend(w, OffsetPtr(sendBuf, sent),
                  VAL(INTEGER, len) - sent);
      IF n > 0 THEN
        sent := sent + n
      ELSIF n = -2 THEN
        DEALLOCATE(sendBuf, len);
        RETURN -2
      ELSE
        DEALLOCATE(sendBuf, len);
        RETURN -1
      END
    END;
    DEALLOCATE(sendBuf, len)
  END;

  RETURN 0
END SendFrame;

(* ── Handle received frames ──────────────────────────────── *)

PROCEDURE HandleFrame(w: WsPtr; VAR hdr: FrameHeader;
                      payload: ADDRESS);
VAR
  v: Value;
  e: Error;
  dummy: Promise.Status;
  closeCode: CARDINAL;
BEGIN
  CASE OpcodeToInt(hdr.opcode) OF

    0: (* Continuation *)
      IF EnsureMsgCap(w, w^.msgLen + hdr.payloadLen) THEN
        IF hdr.payloadLen > 0 THEN
          CopyBytes(payload, OffsetPtr(w^.msgBuf, VAL(INTEGER, w^.msgLen)),
                    VAL(INTEGER, hdr.payloadLen));
          w^.msgLen := w^.msgLen + hdr.payloadLen
        END;
        IF hdr.fin THEN
          (* Deliver complete message *)
          IF w^.handler # NIL THEN
            w^.handler(w, w^.msgOpcode, w^.msgBuf, w^.msgLen, w^.handlerCtx)
          END;
          w^.msgLen := 0
        END
      END |

    1, 2: (* Text / Binary *)
      IF hdr.fin THEN
        (* Single-frame message -- deliver directly *)
        IF w^.handler # NIL THEN
          w^.handler(w, hdr.opcode, payload, hdr.payloadLen, w^.handlerCtx)
        END
      ELSE
        (* Start fragmented message *)
        w^.msgOpcode := hdr.opcode;
        w^.msgLen := 0;
        IF EnsureMsgCap(w, hdr.payloadLen) THEN
          IF hdr.payloadLen > 0 THEN
            CopyBytes(payload, w^.msgBuf, VAL(INTEGER, hdr.payloadLen));
            w^.msgLen := hdr.payloadLen
          END
        END
      END |

    8: (* Close *)
      IF w^.state = StOpen THEN
        (* Echo close frame *)
        IF hdr.payloadLen >= 2 THEN
          dummy := VAL(Promise.Status,
                       SendFrame(w, OpClose, TRUE, payload, hdr.payloadLen))
        ELSE
          dummy := VAL(Promise.Status,
                       SendFrame(w, OpClose, TRUE, NIL, 0))
        END;
        w^.state := StClosed;
        IF w^.handler # NIL THEN
          w^.handler(w, OpClose, payload, hdr.payloadLen, w^.handlerCtx)
        END
      ELSIF w^.state = StClosing THEN
        (* We initiated close, got the response *)
        w^.state := StClosed;
        IF w^.closePromise # NIL THEN
          v.tag := 0;
          v.ptr := NIL;
          dummy := Resolve(w^.closePromise, v);
          w^.closePromise := NIL
        END
      END |

    9: (* Ping *)
      (* Reply with Pong *)
      dummy := VAL(Promise.Status,
                   SendFrame(w, OpPong, TRUE, payload, hdr.payloadLen));
      IF w^.handler # NIL THEN
        w^.handler(w, OpPing, payload, hdr.payloadLen, w^.handlerCtx)
      END |

    10: (* Pong *)
      IF w^.handler # NIL THEN
        w^.handler(w, OpPong, payload, hdr.payloadLen, w^.handlerCtx)
      END

  ELSE
    (* Unknown opcode -- ignore *)
  END
END HandleFrame;

(* ── Process received data (frame mode) ──────────────────── *)

PROCEDURE ProcessFrames(w: WsPtr);
VAR
  hdr: FrameHeader;
  st: WsFrame.Status;
  available: CARDINAL;
  payload: ADDRESS;
BEGIN
  WHILE w^.recvPos < w^.recvLen DO
    available := VAL(CARDINAL, w^.recvLen - w^.recvPos);
    st := DecodeHeader(OffsetPtr(ADR(w^.recvBuf), w^.recvPos),
                       available, hdr);
    IF st = WsFrame.Incomplete THEN
      (* Need more data *)
      RETURN
    ELSIF st = WsFrame.Invalid THEN
      FailWs(w, 4);  (* protocol error *)
      RETURN
    END;

    (* Check if we have the full frame *)
    IF available < hdr.headerLen + hdr.payloadLen THEN
      RETURN  (* incomplete payload *)
    END;

    (* Extract payload pointer *)
    IF hdr.payloadLen > 0 THEN
      payload := OffsetPtr(ADR(w^.recvBuf),
                           w^.recvPos + VAL(INTEGER, hdr.headerLen));
      (* Unmask server-sent data if masked (servers should not mask,
         but handle it gracefully) *)
      IF hdr.masked THEN
        ApplyMask(payload, hdr.payloadLen, hdr.maskKey, 0)
      END
    ELSE
      payload := NIL
    END;

    HandleFrame(w, hdr, payload);

    w^.recvPos := w^.recvPos + VAL(INTEGER, hdr.headerLen)
                + VAL(INTEGER, hdr.payloadLen);

    IF w^.state >= StClosed THEN RETURN END
  END;

  (* Compact receive buffer *)
  IF w^.recvPos > 0 THEN
    IF w^.recvPos < w^.recvLen THEN
      CopyBytes(OffsetPtr(ADR(w^.recvBuf), w^.recvPos),
                ADR(w^.recvBuf),
                w^.recvLen - w^.recvPos);
      w^.recvLen := w^.recvLen - w^.recvPos
    ELSE
      w^.recvLen := 0
    END;
    w^.recvPos := 0
  END
END ProcessFrames;

(* ── Upgrade response parsing ────────────────────────────── *)

PROCEDURE ProcessUpgradeResponse(w: WsPtr): BOOLEAN;
VAR
  i, j, available: INTEGER;
  ch: CHAR;
  nameStart, nameEnd, valStart, valEnd: INTEGER;
  name, val: ARRAY [0..255] OF CHAR;
  nameLen, valLen: INTEGER;
  upgradeStr, wsStr: ARRAY [0..15] OF CHAR;
  upgradeLen, wsLen: INTEGER;
BEGIN
  upgradeStr := "upgrade";
  upgradeLen := 7;
  wsStr := "websocket";
  wsLen := 9;

  (* Parse lines from recvBuf *)
  WHILE w^.recvPos < w^.recvLen DO
    ch := w^.recvBuf[w^.recvPos];
    INC(w^.recvPos);

    IF ch = CHR(10) THEN
      (* End of line *)
      (* Strip trailing CR *)
      IF (w^.lineLen > 0) AND (w^.lineBuf[w^.lineLen - 1] = CHR(13)) THEN
        DEC(w^.lineLen)
      END;

      IF NOT w^.gotStatus THEN
        (* First line: "HTTP/1.1 101 ..." *)
        w^.gotStatus := TRUE;
        (* Check for "101" in status line *)
        w^.statusOk := FALSE;
        IF w^.lineLen >= 12 THEN
          IF (w^.lineBuf[9] = '1') AND (w^.lineBuf[10] = '0')
             AND (w^.lineBuf[11] = '1') THEN
            w^.statusOk := TRUE
          END
        END
      ELSIF w^.lineLen = 0 THEN
        (* Empty line = end of headers *)
        IF w^.statusOk AND w^.gotUpgrade AND w^.gotAccept THEN
          RETURN TRUE  (* success *)
        ELSE
          RETURN FALSE  (* bad handshake *)
        END
      ELSE
        (* Header line: find colon *)
        i := 0;
        WHILE (i < w^.lineLen) AND (w^.lineBuf[i] # ':') DO INC(i) END;
        IF i < w^.lineLen THEN
          (* Extract name *)
          nameLen := i;
          IF nameLen > 255 THEN nameLen := 255 END;
          FOR j := 0 TO nameLen - 1 DO
            name[j] := w^.lineBuf[j]
          END;
          (* Skip colon and spaces *)
          INC(i);
          WHILE (i < w^.lineLen) AND (w^.lineBuf[i] = ' ') DO INC(i) END;
          (* Extract value *)
          valLen := w^.lineLen - i;
          IF valLen > 255 THEN valLen := 255 END;
          FOR j := 0 TO valLen - 1 DO
            val[j] := w^.lineBuf[i + j]
          END;

          (* Check headers *)
          IF StrEqCI(name, upgradeStr, nameLen, upgradeLen) THEN
            IF StrEqCI(val, wsStr, valLen, wsLen) THEN
              w^.gotUpgrade := TRUE
            END
          END;

          IF nameLen = 20 THEN
            (* "Sec-WebSocket-Accept" *)
            name[nameLen] := 0C;
            IF StrEqCI(name,
                       "sec-websocket-accept",
                       nameLen, 20) THEN
              (* Compare accept value *)
              IF valLen = w^.expectedAcceptLen THEN
                w^.gotAccept := TRUE;
                FOR j := 0 TO valLen - 1 DO
                  IF val[j] # w^.expectedAccept[j] THEN
                    w^.gotAccept := FALSE
                  END
                END
              END
            END
          END
        END
      END;

      w^.lineLen := 0
    ELSIF ch # CHR(13) THEN
      IF w^.lineLen < LineBufSize THEN
        w^.lineBuf[w^.lineLen] := ch;
        INC(w^.lineLen)
      END
    END
  END;

  (* Need more data *)
  RETURN FALSE
END ProcessUpgradeResponse;

(* ── Event handler ───────────────────────────────────────── *)

PROCEDURE OnWsEvent(fd, events: INTEGER; user: ADDRESS);
VAR
  w: WsPtr;
  n, err: INTEGER;
  v: Value;
  e: Error;
  dummy: Promise.Status;
  est: EventLoop.Status;
  tst: TLS.Status;
  sst: Stream.Status;
  handshakeDone: BOOLEAN;
BEGIN
  w := user;

  CASE w^.state OF

    StConnecting:
      (* Check connect result *)
      err := m2_getsockopt_error(w^.sock);
      IF err # 0 THEN
        FailWs(w, 2);
        RETURN
      END;

      IF w^.useTLS THEN
        (* TCP connected -- begin TLS handshake *)
        w^.state := StHandshaking;
        tst := TLS.Handshake(w^.tlsSess);
        IF tst = TLS.OK THEN
          sst := Stream.CreateTLS(w^.loop, w^.sched,
                                  w^.sock, w^.tlsCtx, w^.tlsSess,
                                  w^.stream);
          IF sst # Stream.OK THEN
            FailWs(w, 2);
            RETURN
          END;
          w^.tlsCtx := NIL;
          w^.tlsSess := NIL;
          w^.state := StSending;
          est := EventLoop.ModifyFd(w^.loop, w^.sock, EvWrite)
        ELSIF tst = TLS.WantRead THEN
          est := EventLoop.ModifyFd(w^.loop, w^.sock, EvRead)
        ELSIF tst = TLS.WantWrite THEN
          est := EventLoop.ModifyFd(w^.loop, w^.sock, EvWrite)
        ELSE
          FailWs(w, 2)
        END;
        RETURN
      END;

      (* TCP connected (no TLS) -- create stream and send upgrade *)
      sst := Stream.CreateTCP(w^.loop, w^.sched, w^.sock, w^.stream);
      IF sst # Stream.OK THEN
        FailWs(w, 2);
        RETURN
      END;

      w^.state := StSending;
      est := EventLoop.ModifyFd(w^.loop, w^.sock, EvWrite) |

    StHandshaking:
      (* TLS handshake continuation *)
      tst := TLS.Handshake(w^.tlsSess);
      IF tst = TLS.OK THEN
        (* Handshake complete -- create TLS stream and start sending *)
        sst := Stream.CreateTLS(w^.loop, w^.sched,
                                w^.sock, w^.tlsCtx, w^.tlsSess,
                                w^.stream);
        IF sst # Stream.OK THEN
          FailWs(w, 2);
          RETURN
        END;
        (* Stream owns TLS resources now *)
        w^.tlsCtx := NIL;
        w^.tlsSess := NIL;
        w^.state := StSending;
        est := EventLoop.ModifyFd(w^.loop, w^.sock, EvWrite)
      ELSIF tst = TLS.WantRead THEN
        est := EventLoop.ModifyFd(w^.loop, w^.sock, EvRead)
      ELSIF tst = TLS.WantWrite THEN
        est := EventLoop.ModifyFd(w^.loop, w^.sock, EvWrite)
      ELSE
        FailWs(w, 2)
      END |

    StSending:
      (* Send upgrade request *)
      n := DoSend(w, OffsetPtr(ADR(w^.request), w^.reqSent),
                  w^.reqLen - w^.reqSent);
      IF n > 0 THEN
        w^.reqSent := w^.reqSent + n;
        IF w^.reqSent >= w^.reqLen THEN
          (* All sent -- switch to receiving *)
          w^.state := StRecvUpgrade;
          w^.recvLen := 0;
          w^.recvPos := 0;
          w^.lineLen := 0;
          w^.gotStatus := FALSE;
          w^.gotUpgrade := FALSE;
          w^.gotAccept := FALSE;
          w^.statusOk := FALSE;
          est := EventLoop.ModifyFd(w^.loop, w^.sock, EvRead)
        END
      ELSIF n = -2 THEN
        (* would block *)
      ELSE
        FailWs(w, 2)
      END |

    StRecvUpgrade:
      (* Receive upgrade response *)
      n := DoRecv(w, OffsetPtr(ADR(w^.recvBuf), w^.recvLen),
                  RecvBufSize - w^.recvLen);
      IF n > 0 THEN
        w^.recvLen := w^.recvLen + n;
        w^.recvPos := 0;
        handshakeDone := ProcessUpgradeResponse(w);
        IF handshakeDone THEN
          (* Connected! *)
          w^.state := StOpen;
          w^.recvLen := 0;
          w^.recvPos := 0;
          v.tag := 0;
          v.ptr := w;
          IF w^.promise # NIL THEN
            dummy := Resolve(w^.promise, v);
            w^.promise := NIL
          END;
          est := EventLoop.ModifyFd(w^.loop, w^.sock, EvRead)
        ELSIF w^.gotStatus AND (NOT w^.statusOk) THEN
          FailWs(w, 3)  (* not 101 *)
        END
        (* else need more data *)
      ELSIF n = 0 THEN
        FailWs(w, 2)
      ELSIF n = -2 THEN
        (* would block *)
      ELSE
        FailWs(w, 2)
      END |

    StOpen, StClosing:
      (* Read frames *)
      n := DoRecv(w, OffsetPtr(ADR(w^.recvBuf), w^.recvLen),
                  RecvBufSize - w^.recvLen);
      IF n > 0 THEN
        w^.recvLen := w^.recvLen + n;
        w^.recvPos := 0;
        ProcessFrames(w)
      ELSIF n = 0 THEN
        (* Connection closed *)
        w^.state := StClosed;
        IF w^.closePromise # NIL THEN
          v.tag := 0;
          v.ptr := NIL;
          dummy := Resolve(w^.closePromise, v);
          w^.closePromise := NIL
        END
      ELSIF n = -2 THEN
        (* would block *)
      ELSE
        FailWs(w, 1)
      END

  ELSE
    (* StClosed / StError -- ignore *)
  END
END OnWsEvent;

(* ── Public API ──────────────────────────────────────────── *)

PROCEDURE Connect(lp: Loop; sched: Scheduler;
                  url: ARRAY OF CHAR; VAR ws: WebSocket): Future;
VAR
  w: WsPtr;
  uri: URIRec;
  ust: URI.Status;
  f: Future;
  dnsFuture: Future;
  dnsSettled: BOOLEAN;
  dnsResult: Result;
  pst: Promise.Status;
  ap: AddrPtr;
  dst: DNS.Status;
  sock: Socket;
  sst: Sockets.Status;
  rc: INTEGER;
  est: EventLoop.Status;
  tst: TLS.Status;
  isWss: BOOLEAN;
  scheme: ARRAY [0..5] OF CHAR;
BEGIN
  (* Parse URL *)
  ust := URI.Parse(url, uri);
  IF ust # URI.OK THEN
    ws := NIL;
    RETURN NIL
  END;

  (* Determine scheme *)
  isWss := FALSE;
  scheme := "wss";
  IF StrEqCI(uri.scheme, scheme, uri.schemeLen, 3) THEN
    isWss := TRUE
  ELSE
    scheme := "ws";
    IF NOT StrEqCI(uri.scheme, scheme, uri.schemeLen, 2) THEN
      ws := NIL;
      RETURN NIL
    END
  END;

  (* Default ports *)
  IF uri.port = 0 THEN
    IF isWss THEN uri.port := 443
    ELSE uri.port := 80
    END
  END;

  (* DNS resolve (synchronous) *)
  dst := DNS.ResolveA(lp, sched, uri.host, uri.port, dnsFuture);
  IF dst # DNS.OK THEN
    ws := NIL;
    RETURN NIL
  END;
  pst := GetResultIfSettled(dnsFuture, dnsSettled, dnsResult);
  IF (NOT dnsSettled) OR (NOT dnsResult.isOk) THEN
    ws := NIL;
    RETURN NIL
  END;
  ap := dnsResult.v.ptr;

  (* Create socket *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, sock);
  IF sst # Sockets.OK THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    ws := NIL;
    RETURN NIL
  END;
  sst := Sockets.SetNonBlocking(sock, TRUE);

  (* Allocate state *)
  ALLOCATE(w, TSIZE(WsRec));
  IF w = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    sst := Sockets.CloseSocket(sock);
    ws := NIL;
    RETURN NIL
  END;

  w^.state := StConnecting;
  w^.sock := sock;
  w^.loop := lp;
  w^.sched := sched;
  w^.useTLS := isWss;
  w^.tlsCtx := NIL;
  w^.tlsSess := NIL;
  w^.stream := NIL;
  w^.reqLen := 0;
  w^.reqSent := 0;
  w^.recvLen := 0;
  w^.recvPos := 0;
  w^.lineLen := 0;
  w^.gotStatus := FALSE;
  w^.gotUpgrade := FALSE;
  w^.gotAccept := FALSE;
  w^.statusOk := FALSE;
  w^.msgBuf := NIL;
  w^.msgLen := 0;
  w^.msgCap := 0;
  w^.handler := NIL;
  w^.handlerCtx := NIL;
  w^.closePromise := NIL;

  (* Generate WebSocket key and compute expected accept *)
  GenerateKey(w);
  ComputeExpectedAccept(w);

  (* Build upgrade request *)
  BuildUpgradeRequest(w, uri);

  (* Create promise *)
  pst := PromiseCreate(sched, w^.promise, f);
  IF pst # Promise.OK THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    sst := Sockets.CloseSocket(sock);
    DEALLOCATE(w, TSIZE(WsRec));
    ws := NIL;
    RETURN NIL
  END;

  (* Set up TLS if needed *)
  IF isWss THEN
    tst := TLS.ContextCreate(w^.tlsCtx);
    IF tst # TLS.OK THEN
      DEALLOCATE(ap, TSIZE(AddrRec));
      FailWs(w, 2);
      ws := w;
      RETURN f
    END;
    tst := TLS.SetVerifyMode(w^.tlsCtx, TLS.VerifyPeer);
    tst := TLS.SetMinVersion(w^.tlsCtx, TLS.TLS12);
    tst := TLS.LoadSystemRoots(w^.tlsCtx);
    IF tst # TLS.OK THEN
      DEALLOCATE(ap, TSIZE(AddrRec));
      FailWs(w, 2);
      ws := w;
      RETURN f
    END;
    tst := TLS.SessionCreate(lp, sched, w^.tlsCtx, sock, w^.tlsSess);
    IF tst # TLS.OK THEN
      DEALLOCATE(ap, TSIZE(AddrRec));
      FailWs(w, 2);
      ws := w;
      RETURN f
    END;
    tst := TLS.SetSNI(w^.tlsSess, uri.host)
  END;

  (* Start non-blocking connect *)
  rc := m2_connect_ipv4(sock,
                        ORD(ap^.addrV4[0]),
                        ORD(ap^.addrV4[1]),
                        ORD(ap^.addrV4[2]),
                        ORD(ap^.addrV4[3]),
                        uri.port);
  DEALLOCATE(ap, TSIZE(AddrRec));

  IF rc < 0 THEN
    FailWs(w, 2);
    ws := w;
    RETURN f
  END;

  (* Watch for connect completion *)
  est := EventLoop.WatchFd(lp, sock, EvWrite, OnWsEvent, w);
  IF est # EventLoop.OK THEN
    FailWs(w, 2);
    ws := w;
    RETURN f
  END;

  (* If connect completed immediately (non-TLS) *)
  IF (rc = 0) AND (NOT isWss) THEN
    OnWsEvent(sock, EvWrite, w)
  END;

  ws := w;
  RETURN f
END Connect;

PROCEDURE Close(ws: WebSocket; code: CARDINAL;
                reason: ARRAY OF CHAR): Future;
VAR
  w: WsPtr;
  f: Future;
  pst: Promise.Status;
  closeBuf: ARRAY [0..127] OF CHAR;
  closeLen: INTEGER;
  i, rlen: INTEGER;
  rc: INTEGER;
BEGIN
  w := ws;
  IF w = NIL THEN RETURN NIL END;
  IF w^.state # StOpen THEN RETURN NIL END;

  pst := PromiseCreate(w^.sched, w^.closePromise, f);
  IF pst # Promise.OK THEN RETURN NIL END;

  (* Build close payload: 2-byte status code + reason *)
  closeBuf[0] := CHR(code DIV 256);
  closeBuf[1] := CHR(code MOD 256);
  closeLen := 2;

  rlen := StrLen(reason);
  IF rlen > 123 THEN rlen := 123 END;  (* control frame max 125 bytes *)
  FOR i := 0 TO rlen - 1 DO
    closeBuf[closeLen] := reason[i];
    INC(closeLen)
  END;

  w^.state := StClosing;
  rc := SendFrame(w, OpClose, TRUE, ADR(closeBuf), VAL(CARDINAL, closeLen));
  IF rc < 0 THEN
    FailWs(w, 1);
    RETURN f
  END;

  RETURN f
END Close;

PROCEDURE Destroy(ws: WebSocket);
VAR w: WsPtr;
BEGIN
  w := ws;
  IF w = NIL THEN RETURN END;
  CleanupWs(w);
  DEALLOCATE(w, TSIZE(WsRec))
END Destroy;

PROCEDURE Send(ws: WebSocket; opcode: Opcode;
               data: ADDRESS; len: CARDINAL): Future;
VAR
  w: WsPtr;
  f: Future;
  p: Promise;
  pst: Promise.Status;
  v: Value;
  e: Error;
  rc: INTEGER;
BEGIN
  w := ws;
  IF w = NIL THEN RETURN NIL END;
  IF w^.state # StOpen THEN RETURN NIL END;

  pst := PromiseCreate(w^.sched, p, f);
  IF pst # Promise.OK THEN RETURN NIL END;

  rc := SendFrame(w, opcode, TRUE, data, len);
  IF rc = 0 THEN
    v.tag := VAL(INTEGER, len);
    v.ptr := NIL;
    pst := Resolve(p, v)
  ELSE
    e.code := 1;
    e.ptr := NIL;
    pst := Reject(p, e)
  END;

  RETURN f
END Send;

PROCEDURE SendText(ws: WebSocket; text: ARRAY OF CHAR): Future;
VAR len: INTEGER;
BEGIN
  len := StrLen(text);
  RETURN Send(ws, OpText, ADR(text), VAL(CARDINAL, len))
END SendText;

PROCEDURE OnMessage(ws: WebSocket; handler: MessageProc; ctx: ADDRESS);
VAR w: WsPtr;
BEGIN
  w := ws;
  IF w = NIL THEN RETURN END;
  w^.handler := handler;
  w^.handlerCtx := ctx
END OnMessage;

PROCEDURE GetState(ws: WebSocket): State;
VAR w: WsPtr;
BEGIN
  w := ws;
  IF w = NIL THEN RETURN Closed END;
  CASE w^.state OF
    StConnecting, StSending, StRecvUpgrade, StHandshaking:
      RETURN Connecting |
    StOpen:
      RETURN Open |
    StClosing:
      RETURN Closing |
    StClosed, StError:
      RETURN Closed
  ELSE
    RETURN Closed
  END
END GetState;

BEGIN
  (* no module initialization needed *)
END WebSocket.
