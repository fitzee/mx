IMPLEMENTATION MODULE HTTPClient;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Promise, Value, Error,
                    PromiseCreate, Resolve, Reject,
                    GetResultIfSettled, Result;
IMPORT Promise;
FROM Poller IMPORT EvRead, EvWrite;
IMPORT EventLoop;
IMPORT Buffers;
FROM URI IMPORT URIRec, RequestPath;
IMPORT URI;
IMPORT DNS;
FROM DNS IMPORT AddrRec, AddrPtr;
FROM Sockets IMPORT Socket, InvalidSocket, AF_INET, SOCK_STREAM,
                    SocketCreate, CloseSocket, SetNonBlocking;
IMPORT Sockets;
FROM DnsBridge IMPORT m2_connect_ipv4, m2_getsockopt_error;
IMPORT TLS;
IMPORT Stream;

(* ── Connection states ─────────────────────────────────────────── *)

CONST
  StConnecting   = 0;
  StSending      = 1;
  StRecvStatus   = 2;
  StRecvHeaders  = 3;
  StRecvBody     = 4;
  StDone         = 5;
  StError        = 6;
  StHandshaking  = 7;

  MaxReqSize = 4096;
  RecvBufCap = 8192;
  LineBufSize = 2048;

  (* Chunked sub-states *)
  ChSize    = 0;
  ChData    = 1;
  ChTrailer = 2;
  ChDone    = 3;

TYPE
  ConnRec = RECORD
    state      : INTEGER;
    sock       : Socket;
    promise    : Promise;
    loop       : EventLoop.Loop;
    sched      : Scheduler;
    recvBuf    : Buffers.Buffer;
    resp       : ResponsePtr;
    request    : ARRAY [0..MaxReqSize-1] OF CHAR;
    reqLen     : INTEGER;
    reqSent    : INTEGER;
    contentLen : INTEGER;
    bodyRead   : INTEGER;
    chunked    : BOOLEAN;
    headOnly   : BOOLEAN;
    chunkState : INTEGER;
    chunkRem   : INTEGER;
    (* Request body (for PUT/POST) *)
    reqBody    : ADDRESS;
    reqBodyLen : INTEGER;
    reqBodySent: INTEGER;
    (* Transport *)
    useTLS     : BOOLEAN;
    tlsCtx     : TLS.TLSContext;
    tlsSess    : TLS.TLSSession;
    stream     : Stream.Stream;
  END;

  ConnPtr = POINTER TO ConnRec;

VAR
  mSkipVerify: BOOLEAN;

(* ── String helpers ────────────────────────────────────────────── *)

PROCEDURE AppendCh(c: ConnPtr; ch: CHAR);
BEGIN
  IF c^.reqLen < MaxReqSize THEN
    c^.request[c^.reqLen] := ch;
    INC(c^.reqLen)
  END
END AppendCh;

PROCEDURE AppendStr(c: ConnPtr; VAR s: ARRAY OF CHAR);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    AppendCh(c, s[i]);
    INC(i)
  END
END AppendStr;

PROCEDURE AppendCRLF(c: ConnPtr);
BEGIN
  AppendCh(c, CHR(13));
  AppendCh(c, CHR(10))
END AppendCRLF;

PROCEDURE StrToInt(VAR s: ARRAY OF CHAR; len: INTEGER;
                   VAR val: INTEGER): BOOLEAN;
VAR i: INTEGER;
BEGIN
  val := 0;
  i := 0;
  (* skip leading spaces *)
  WHILE (i < len) AND (s[i] = ' ') DO INC(i) END;
  IF i >= len THEN RETURN FALSE END;
  WHILE (i < len) AND (s[i] >= '0') AND (s[i] <= '9') DO
    val := val * 10 + (ORD(s[i]) - ORD('0'));
    INC(i)
  END;
  RETURN TRUE
END StrToInt;

PROCEDURE HexToInt(VAR s: ARRAY OF CHAR; len: INTEGER;
                   VAR val: INTEGER): BOOLEAN;
VAR i, d: INTEGER; ch: CHAR;
BEGIN
  val := 0;
  i := 0;
  WHILE (i < len) AND (s[i] = ' ') DO INC(i) END;
  IF i >= len THEN RETURN FALSE END;
  WHILE i < len DO
    ch := s[i];
    IF (ch >= '0') AND (ch <= '9') THEN
      d := ORD(ch) - ORD('0')
    ELSIF (ch >= 'a') AND (ch <= 'f') THEN
      d := ORD(ch) - ORD('a') + 10
    ELSIF (ch >= 'A') AND (ch <= 'F') THEN
      d := ORD(ch) - ORD('A') + 10
    ELSIF (ch = CHR(13)) OR (ch = ';') THEN
      (* stop at CR or chunk extension *)
      RETURN TRUE
    ELSE
      RETURN FALSE
    END;
    val := val * 16 + d;
    INC(i)
  END;
  RETURN TRUE
END HexToInt;

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

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): INTEGER;
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
  RETURN i
END StrLen;

(* ── TLS-aware I/O helpers ─────────────────────────────────────── *)

(* DoSend: send bytes through Stream (TCP or TLS).
   Returns >0 bytes sent, -1 error, -2 would-block (watcher adjusted). *)
PROCEDURE DoSend(c: ConnPtr; buf: ADDRESS; len: INTEGER): INTEGER;
VAR n: INTEGER; st: Stream.Status;
BEGIN
  st := Stream.TryWrite(c^.stream, buf, len, n);
  IF st = Stream.OK THEN RETURN n
  ELSIF st = Stream.WouldBlock THEN RETURN -2
  ELSE RETURN -1
  END
END DoSend;

(* DoRecv: receive bytes through Stream (TCP or TLS).
   Returns >0 bytes, 0 closed, -1 error, -2 would-block (watcher adjusted). *)
PROCEDURE DoRecv(c: ConnPtr; buf: ADDRESS; max: INTEGER): INTEGER;
VAR n: INTEGER; st: Stream.Status;
BEGIN
  st := Stream.TryRead(c^.stream, buf, max, n);
  IF st = Stream.OK THEN RETURN n
  ELSIF st = Stream.StreamClosed THEN RETURN 0
  ELSIF st = Stream.WouldBlock THEN RETURN -2
  ELSE RETURN -1
  END
END DoRecv;

(* ── Cleanup ───────────────────────────────────────────────────── *)

PROCEDURE CleanupConn(c: ConnPtr);
VAR est: EventLoop.Status; sst: Sockets.Status;
    bst: Buffers.Status; tst: TLS.Status;
    stst: Stream.Status;
BEGIN
  (* Stream cleanup — handles TLS shutdown + session/ctx destroy *)
  IF c^.stream # NIL THEN
    stst := Stream.Destroy(c^.stream);
    c^.stream := NIL
  END;
  (* Pre-stream TLS cleanup (if failed during handshake) *)
  IF c^.useTLS THEN
    IF c^.tlsSess # NIL THEN
      tst := TLS.Shutdown(c^.tlsSess);
      tst := TLS.SessionDestroy(c^.tlsSess);
      c^.tlsSess := NIL
    END;
    IF c^.tlsCtx # NIL THEN
      tst := TLS.ContextDestroy(c^.tlsCtx);
      c^.tlsCtx := NIL
    END
  END;
  IF c^.sock # InvalidSocket THEN
    est := EventLoop.UnwatchFd(c^.loop, c^.sock);
    sst := Sockets.CloseSocket(c^.sock);
    c^.sock := InvalidSocket
  END;
  IF c^.recvBuf # NIL THEN
    bst := Buffers.Destroy(c^.recvBuf);
    c^.recvBuf := NIL
  END
END CleanupConn;

PROCEDURE FailConn(c: ConnPtr; code: INTEGER);
VAR e: Error; dummy: Promise.Status; bst: Buffers.Status;
BEGIN
  CleanupConn(c);
  e.code := code;
  e.ptr := NIL;
  dummy := Reject(c^.promise, e);
  c^.state := StError;
  (* Free response if promise rejected *)
  IF c^.resp # NIL THEN
    IF c^.resp^.body # NIL THEN
      bst := Buffers.Destroy(c^.resp^.body);
      c^.resp^.body := NIL
    END;
    DEALLOCATE(c^.resp, TSIZE(Response));
    c^.resp := NIL
  END;
  DEALLOCATE(c, TSIZE(ConnRec))
END FailConn;

PROCEDURE SucceedConn(c: ConnPtr);
VAR v: Value; dummy: Promise.Status;
BEGIN
  (* Transfer recvBuf to response body *)
  IF c^.resp^.body = NIL THEN
    c^.resp^.body := c^.recvBuf;
    c^.recvBuf := NIL
  END;
  CleanupConn(c);
  v.tag := 0;
  v.ptr := c^.resp;
  dummy := Resolve(c^.promise, v);
  c^.state := StDone;
  c^.resp := NIL;
  DEALLOCATE(c, TSIZE(ConnRec))
END SucceedConn;

(* ── Request building ──────────────────────────────────────────── *)

PROCEDURE BuildRequest(c: ConnPtr; VAR method: ARRAY OF CHAR;
                       VAR uri: URIRec);
VAR
  rpath: ARRAY [0..2047] OF CHAR;
  rpLen: INTEGER;
  ust: URI.Status;
BEGIN
  c^.reqLen := 0;
  AppendStr(c, method);
  AppendCh(c, ' ');

  ust := RequestPath(uri, rpath, rpLen);
  IF rpLen > 0 THEN
    AppendStr(c, rpath)
  ELSE
    AppendCh(c, '/')
  END;

  AppendStr(c, " HTTP/1.1");
  AppendCRLF(c);
  AppendStr(c, "Host: ");
  AppendStr(c, uri.host);
  AppendCRLF(c);
  AppendStr(c, "Connection: close");
  AppendCRLF(c);
  AppendStr(c, "User-Agent: m2http/0.1");
  AppendCRLF(c);
  AppendCRLF(c)
END BuildRequest;

PROCEDURE IntToStr(val: INTEGER; VAR buf: ARRAY OF CHAR);
VAR
  tmp: ARRAY [0..15] OF CHAR;
  i, j, v: INTEGER;
BEGIN
  IF val = 0 THEN
    buf[0] := '0'; buf[1] := 0C;
    RETURN
  END;
  v := val;
  i := 0;
  WHILE (v > 0) AND (i < 15) DO
    tmp[i] := CHR(ORD('0') + (v MOD 10));
    v := v DIV 10;
    INC(i)
  END;
  (* Reverse into buf *)
  j := 0;
  WHILE i > 0 DO
    DEC(i);
    IF j <= HIGH(buf) THEN
      buf[j] := tmp[i]; INC(j)
    END
  END;
  IF j <= HIGH(buf) THEN buf[j] := 0C END
END IntToStr;

PROCEDURE BuildRequestWithBody(c: ConnPtr; VAR method: ARRAY OF CHAR;
                                VAR uri: URIRec;
                                bodyLen: INTEGER;
                                VAR contentType: ARRAY OF CHAR;
                                VAR authorization: ARRAY OF CHAR);
VAR
  rpath: ARRAY [0..2047] OF CHAR;
  rpLen: INTEGER;
  ust: URI.Status;
  clBuf: ARRAY [0..15] OF CHAR;
BEGIN
  c^.reqLen := 0;
  AppendStr(c, method);
  AppendCh(c, ' ');

  ust := RequestPath(uri, rpath, rpLen);
  IF rpLen > 0 THEN
    AppendStr(c, rpath)
  ELSE
    AppendCh(c, '/')
  END;

  AppendStr(c, " HTTP/1.1");
  AppendCRLF(c);
  AppendStr(c, "Host: ");
  AppendStr(c, uri.host);
  AppendCRLF(c);
  AppendStr(c, "Connection: close");
  AppendCRLF(c);
  AppendStr(c, "User-Agent: m2http/0.1");
  AppendCRLF(c);
  (* Content-Type *)
  IF StrLen(contentType) > 0 THEN
    AppendStr(c, "Content-Type: ");
    AppendStr(c, contentType);
    AppendCRLF(c)
  END;
  (* Content-Length *)
  IntToStr(bodyLen, clBuf);
  AppendStr(c, "Content-Length: ");
  AppendStr(c, clBuf);
  AppendCRLF(c);
  (* Authorization *)
  IF StrLen(authorization) > 0 THEN
    AppendStr(c, "Authorization: ");
    AppendStr(c, authorization);
    AppendCRLF(c)
  END;
  AppendCRLF(c)
END BuildRequestWithBody;

(* ── Response parsing ──────────────────────────────────────────── *)

PROCEDURE ParseStatusLine(c: ConnPtr): BOOLEAN;
VAR
  line: ARRAY [0..LineBufSize-1] OF CHAR;
  pos, i, code: INTEGER;
  bst: Buffers.Status;
BEGIN
  IF NOT Buffers.FindCRLF(c^.recvBuf, pos) THEN RETURN FALSE END;
  IF pos >= LineBufSize THEN
    FailConn(c, 5);
    RETURN FALSE
  END;
  bst := Buffers.CopyOut(c^.recvBuf, 0, pos, line);
  bst := Buffers.Consume(c^.recvBuf, pos + 2);

  (* Expect "HTTP/1.x SSS ..." *)
  IF pos < 12 THEN
    FailConn(c, 5);
    RETURN FALSE
  END;
  IF (line[0] # 'H') OR (line[1] # 'T') OR (line[2] # 'T') OR
     (line[3] # 'P') OR (line[4] # '/') THEN
    FailConn(c, 5);
    RETURN FALSE
  END;

  (* Parse status code at position 9..11 *)
  i := 9;
  code := 0;
  WHILE (i < pos) AND (line[i] >= '0') AND (line[i] <= '9') DO
    code := code * 10 + (ORD(line[i]) - ORD('0'));
    INC(i)
  END;
  c^.resp^.statusCode := code;
  RETURN TRUE
END ParseStatusLine;

PROCEDURE ParseHeaders(c: ConnPtr): BOOLEAN;
VAR
  line: ARRAY [0..LineBufSize-1] OF CHAR;
  pos, i, nameLen, valStart, valLen, hc: INTEGER;
  bst: Buffers.Status;
  clName: ARRAY [0..15] OF CHAR;
  teName: ARRAY [0..20] OF CHAR;
  chVal: ARRAY [0..10] OF CHAR;
BEGIN
  clName := "content-length";
  teName := "transfer-encoding";
  chVal := "chunked";

  LOOP
    IF NOT Buffers.FindCRLF(c^.recvBuf, pos) THEN RETURN FALSE END;

    (* Empty line = end of headers *)
    IF pos = 0 THEN
      bst := Buffers.Consume(c^.recvBuf, 2);
      RETURN TRUE
    END;

    IF pos >= LineBufSize THEN
      bst := Buffers.Consume(c^.recvBuf, pos + 2);
      (* skip oversized header *)
    ELSE
      bst := Buffers.CopyOut(c^.recvBuf, 0, pos, line);
      bst := Buffers.Consume(c^.recvBuf, pos + 2);

      (* Find colon *)
      nameLen := 0;
      WHILE (nameLen < pos) AND (line[nameLen] # ':') DO
        INC(nameLen)
      END;

      IF nameLen < pos THEN
        (* Skip ': ' *)
        valStart := nameLen + 1;
        WHILE (valStart < pos) AND (line[valStart] = ' ') DO
          INC(valStart)
        END;
        valLen := pos - valStart;

        (* Store header *)
        hc := c^.resp^.headerCount;
        IF hc < MaxHeaders THEN
          IF nameLen >= MaxHeaderName THEN
            nameLen := MaxHeaderName - 1
          END;
          FOR i := 0 TO nameLen - 1 DO
            c^.resp^.headers[hc].name[i] := line[i]
          END;
          c^.resp^.headers[hc].name[nameLen] := 0C;
          c^.resp^.headers[hc].nameLen := nameLen;

          IF valLen >= MaxHeaderVal THEN
            valLen := MaxHeaderVal - 1
          END;
          FOR i := 0 TO valLen - 1 DO
            c^.resp^.headers[hc].value[i] := line[valStart + i]
          END;
          c^.resp^.headers[hc].value[valLen] := 0C;
          c^.resp^.headers[hc].valueLen := valLen;
          INC(c^.resp^.headerCount)
        END;

        (* Check Content-Length *)
        IF StrEqCI(line, clName, nameLen, 14) THEN
          StrToInt(line, pos, c^.contentLen);
          (* reparse from valStart *)
          c^.contentLen := 0;
          FOR i := valStart TO pos - 1 DO
            IF (line[i] >= '0') AND (line[i] <= '9') THEN
              c^.contentLen := c^.contentLen * 10 +
                               (ORD(line[i]) - ORD('0'))
            END
          END;
          c^.resp^.contentLength := c^.contentLen
        END;

        (* Check Transfer-Encoding: chunked *)
        IF StrEqCI(line, teName, nameLen, 17) THEN
          IF (valLen >= 7) AND
             StrEqCI(line, chVal, valLen, 7) THEN
            c^.chunked := TRUE
          END
        END
      END
    END
  END
END ParseHeaders;

(* ── Chunked body decoding ─────────────────────────────────────── *)

PROCEDURE ProcessChunked(c: ConnPtr): BOOLEAN;
(* Returns TRUE when the final chunk (size 0) is received. *)
VAR
  pos, chunkSz, avail, toCopy, i: INTEGER;
  line: ARRAY [0..63] OF CHAR;
  bst: Buffers.Status;
  bodyBuf: Buffers.Buffer;
  ch: CHAR;
BEGIN
  bodyBuf := c^.resp^.body;

  LOOP
    CASE c^.chunkState OF
      ChSize:
        IF NOT Buffers.FindCRLF(c^.recvBuf, pos) THEN
          RETURN FALSE
        END;
        IF pos > 63 THEN pos := 63 END;
        bst := Buffers.CopyOut(c^.recvBuf, 0, pos, line);
        bst := Buffers.Consume(c^.recvBuf, pos + 2);
        IF NOT HexToInt(line, pos, chunkSz) THEN
          FailConn(c, 5);
          RETURN FALSE
        END;
        IF chunkSz = 0 THEN
          c^.chunkState := ChDone;
          RETURN TRUE
        END;
        c^.chunkRem := chunkSz;
        c^.chunkState := ChData |

      ChData:
        avail := Buffers.Length(c^.recvBuf);
        IF avail = 0 THEN RETURN FALSE END;
        toCopy := c^.chunkRem;
        IF toCopy > avail THEN toCopy := avail END;
        (* Copy bytes from recvBuf to bodyBuf *)
        FOR i := 0 TO toCopy - 1 DO
          bst := Buffers.PeekByte(c^.recvBuf, i, ch);
          bst := Buffers.AppendByte(bodyBuf, ch)
        END;
        bst := Buffers.Consume(c^.recvBuf, toCopy);
        c^.chunkRem := c^.chunkRem - toCopy;
        c^.bodyRead := c^.bodyRead + toCopy;
        IF c^.chunkRem = 0 THEN
          c^.chunkState := ChTrailer
        END |

      ChTrailer:
        (* Consume the \r\n after chunk data *)
        IF Buffers.Length(c^.recvBuf) < 2 THEN RETURN FALSE END;
        bst := Buffers.Consume(c^.recvBuf, 2);
        c^.chunkState := ChSize |

      ChDone:
        RETURN TRUE
    ELSE
      RETURN FALSE
    END
  END
END ProcessChunked;

(* ── TLS handshake helper ──────────────────────────────────────── *)

PROCEDURE DoTLSHandshake(c: ConnPtr);
VAR hst: TLS.Status; est: EventLoop.Status; sst: Stream.Status;
BEGIN
  hst := TLS.Handshake(c^.tlsSess);
  IF hst = TLS.OK THEN
    (* Handshake complete — create TLS stream and start sending *)
    sst := Stream.CreateTLS(c^.loop, c^.sched, c^.sock,
                             c^.tlsCtx, c^.tlsSess, c^.stream);
    c^.tlsCtx := NIL;   (* ownership transferred to stream *)
    c^.tlsSess := NIL;
    c^.state := StSending;
    est := EventLoop.ModifyFd(c^.loop, c^.sock, EvWrite)
  ELSIF hst = TLS.WantRead THEN
    c^.state := StHandshaking;
    est := EventLoop.ModifyFd(c^.loop, c^.sock, EvRead)
  ELSIF hst = TLS.WantWrite THEN
    c^.state := StHandshaking;
    est := EventLoop.ModifyFd(c^.loop, c^.sock, EvWrite)
  ELSE
    FailConn(c, 6)
  END
END DoTLSHandshake;

(* ── Recv into buffer helper ───────────────────────────────────── *)

PROCEDURE RecvInto(c: ConnPtr; buf: Buffers.Buffer;
                   VAR n: INTEGER): BOOLEAN;
(* Returns TRUE if data was received (n > 0).
   Returns FALSE on error, close, or TLS would-block. *)
VAR bst: Buffers.Status;
BEGIN
  bst := Buffers.Compact(buf);
  n := DoRecv(c, Buffers.WritePtr(buf), Buffers.Remaining(buf));
  IF n > 0 THEN
    bst := Buffers.AdvanceWrite(buf, n);
    RETURN TRUE
  ELSIF n = 0 THEN
    RETURN FALSE   (* closed *)
  ELSIF n = -2 THEN
    RETURN FALSE   (* TLS would-block; watcher already adjusted *)
  ELSE
    FailConn(c, 3);
    RETURN FALSE
  END
END RecvInto;

(* ── Body-to-recv transition ───────────────────────────────────── *)

PROCEDURE TransitionToBody(c: ConnPtr);
VAR avail, i: INTEGER; bst: Buffers.Status; ch: CHAR;
    bodyBuf: Buffers.Buffer;
BEGIN
  IF c^.headOnly THEN
    SucceedConn(c);
    RETURN
  END;
  IF c^.chunked THEN
    bst := Buffers.Create(Buffers.DefaultCap, Buffers.Growable,
                          c^.resp^.body);
    c^.chunkState := ChSize;
    c^.state := StRecvBody;
    IF ProcessChunked(c) THEN
      SucceedConn(c)
    END
  ELSIF c^.contentLen = 0 THEN
    SucceedConn(c)
  ELSE
    bst := Buffers.Create(Buffers.DefaultCap, Buffers.Growable,
                          c^.resp^.body);
    bodyBuf := c^.resp^.body;
    avail := Buffers.Length(c^.recvBuf);
    IF avail > 0 THEN
      FOR i := 0 TO avail - 1 DO
        bst := Buffers.PeekByte(c^.recvBuf, i, ch);
        bst := Buffers.AppendByte(bodyBuf, ch)
      END;
      bst := Buffers.Consume(c^.recvBuf, avail);
      c^.bodyRead := avail
    END;
    c^.state := StRecvBody;
    IF (c^.contentLen > 0) AND (c^.bodyRead >= c^.contentLen) THEN
      SucceedConn(c)
    END
  END
END TransitionToBody;

(* ── Socket event handler ──────────────────────────────────────── *)

PROCEDURE OnSocketEvent(fd, events: INTEGER; user: ADDRESS);
VAR
  c: ConnPtr;
  n, sockErr: INTEGER;
  bst: Buffers.Status;
  est: EventLoop.Status;
  bodyBuf: Buffers.Buffer;
  sst: Stream.Status;
BEGIN
  c := user;

  CASE c^.state OF

    StConnecting:
      (* Check if connect succeeded *)
      sockErr := m2_getsockopt_error(fd);
      IF sockErr # 0 THEN
        FailConn(c, 1);
        RETURN
      END;
      IF c^.useTLS THEN
        DoTLSHandshake(c)
      ELSE
        (* Create TCP stream for I/O *)
        sst := Stream.CreateTCP(c^.loop, c^.sched, fd, c^.stream);
        c^.state := StSending;
        est := EventLoop.ModifyFd(c^.loop, fd, EvWrite);
        (* Send first chunk of headers immediately *)
        n := DoSend(c, ADR(c^.request[c^.reqSent]),
                     c^.reqLen - c^.reqSent);
        IF n > 0 THEN
          c^.reqSent := c^.reqSent + n
        ELSIF n = -2 THEN
          RETURN
        ELSIF n < 0 THEN
          FailConn(c, 2);
          RETURN
        END;
        (* Try sending body if headers done *)
        IF (c^.reqSent >= c^.reqLen) AND
           (c^.reqBodyLen > 0) AND (c^.reqBodySent < c^.reqBodyLen) THEN
          n := DoSend(c,
                 VAL(ADDRESS, LONGCARD(c^.reqBody) + LONGCARD(c^.reqBodySent)),
                 c^.reqBodyLen - c^.reqBodySent);
          IF n > 0 THEN
            c^.reqBodySent := c^.reqBodySent + n
          ELSIF n = -2 THEN
            RETURN
          ELSIF n < 0 THEN
            FailConn(c, 2);
            RETURN
          END
        END;
        IF (c^.reqSent >= c^.reqLen) AND
           (c^.reqBodySent >= c^.reqBodyLen) THEN
          c^.state := StRecvStatus;
          est := EventLoop.ModifyFd(c^.loop, fd, EvRead)
        END
      END |

    StHandshaking:
      DoTLSHandshake(c) |

    StSending:
      IF c^.reqSent < c^.reqLen THEN
        (* Still sending headers *)
        n := DoSend(c, ADR(c^.request[c^.reqSent]),
                     c^.reqLen - c^.reqSent);
        IF n > 0 THEN
          c^.reqSent := c^.reqSent + n
        ELSIF n = -2 THEN
          RETURN
        ELSIF n < 0 THEN
          FailConn(c, 2);
          RETURN
        END
      END;
      IF (c^.reqSent >= c^.reqLen) AND
         (c^.reqBodyLen > 0) AND (c^.reqBodySent < c^.reqBodyLen) THEN
        (* Send body bytes *)
        n := DoSend(c,
               VAL(ADDRESS, LONGCARD(c^.reqBody) + LONGCARD(c^.reqBodySent)),
               c^.reqBodyLen - c^.reqBodySent);
        IF n > 0 THEN
          c^.reqBodySent := c^.reqBodySent + n
        ELSIF n = -2 THEN
          RETURN
        ELSIF n < 0 THEN
          FailConn(c, 2);
          RETURN
        END
      END;
      IF (c^.reqSent >= c^.reqLen) AND
         (c^.reqBodySent >= c^.reqBodyLen) THEN
        c^.state := StRecvStatus;
        est := EventLoop.ModifyFd(c^.loop, fd, EvRead)
      END |

    StRecvStatus:
      IF NOT RecvInto(c, c^.recvBuf, n) THEN
        IF n = 0 THEN FailConn(c, 3) END;
        (* n = -2: TLS would-block; n < 0: already FailConn'd *)
        RETURN
      END;
      IF ParseStatusLine(c) THEN
        c^.state := StRecvHeaders;
        IF ParseHeaders(c) THEN
          TransitionToBody(c)
        END
      END |

    StRecvHeaders:
      IF NOT RecvInto(c, c^.recvBuf, n) THEN
        IF n = 0 THEN FailConn(c, 3) END;
        RETURN
      END;
      IF ParseHeaders(c) THEN
        TransitionToBody(c)
      END |

    StRecvBody:
      IF c^.chunked THEN
        IF NOT RecvInto(c, c^.recvBuf, n) THEN
          IF n = 0 THEN
            SucceedConn(c)
          END;
          RETURN
        END;
        IF ProcessChunked(c) THEN
          SucceedConn(c)
        END
      ELSE
        bodyBuf := c^.resp^.body;
        bst := Buffers.Compact(bodyBuf);
        n := DoRecv(c, Buffers.WritePtr(bodyBuf),
                     Buffers.Remaining(bodyBuf));
        IF n > 0 THEN
          bst := Buffers.AdvanceWrite(bodyBuf, n);
          c^.bodyRead := c^.bodyRead + n;
          IF (c^.contentLen > 0) AND
             (c^.bodyRead >= c^.contentLen) THEN
            SucceedConn(c)
          END
        ELSIF n = 0 THEN
          SucceedConn(c)
        ELSIF n = -2 THEN
          RETURN
        ELSE
          FailConn(c, 3)
        END
      END

  ELSE
    (* StDone, StError — should not receive events *)
  END
END OnSocketEvent;

(* ── Detect HTTPS scheme ───────────────────────────────────────── *)

PROCEDURE IsHTTPS(VAR uri: URIRec): BOOLEAN;
BEGIN
  RETURN (uri.schemeLen = 5) AND
         (uri.scheme[0] = 'h') AND (uri.scheme[1] = 't') AND
         (uri.scheme[2] = 't') AND (uri.scheme[3] = 'p') AND
         (uri.scheme[4] = 's')
END IsHTTPS;

(* ── Core request procedure ────────────────────────────────────── *)

PROCEDURE DoRequest(lp: EventLoop.Loop; sched: Scheduler;
                    VAR uri: URIRec;
                    VAR method: ARRAY OF CHAR;
                    headOnly: BOOLEAN;
                    VAR outFuture: Future): Status;
VAR
  c: ConnPtr;
  dnsFuture: Future;
  dnsSettled: BOOLEAN;
  dnsResult: Result;
  ap: AddrPtr;
  pst: Promise.Status;
  dst: DNS.Status;
  sst: Sockets.Status;
  bst: Buffers.Status;
  est: EventLoop.Status;
  tst: TLS.Status;
  crc: INTEGER;
  i: INTEGER;
  wantTLS: BOOLEAN;
BEGIN
  IF (lp = NIL) OR (sched = NIL) THEN RETURN Invalid END;

  wantTLS := IsHTTPS(uri);

  (* 1. Resolve DNS *)
  dst := DNS.ResolveA(lp, sched, uri.host, uri.port, dnsFuture);
  IF dst # DNS.OK THEN RETURN DNSFailed END;
  pst := GetResultIfSettled(dnsFuture, dnsSettled, dnsResult);
  IF (NOT dnsSettled) OR (NOT dnsResult.isOk) THEN
    RETURN DNSFailed
  END;
  ap := dnsResult.v.ptr;

  (* 2. Allocate connection context *)
  ALLOCATE(c, TSIZE(ConnRec));
  IF c = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    RETURN OutOfMemory
  END;
  c^.state := StConnecting;
  c^.loop := lp;
  c^.sched := sched;
  c^.headOnly := headOnly;
  c^.contentLen := -1;
  c^.bodyRead := 0;
  c^.chunked := FALSE;
  c^.chunkState := ChSize;
  c^.chunkRem := 0;
  c^.reqSent := 0;
  c^.reqBody := NIL;
  c^.reqBodyLen := 0;
  c^.reqBodySent := 0;
  c^.sock := InvalidSocket;
  c^.recvBuf := NIL;
  c^.resp := NIL;
  c^.useTLS := wantTLS;
  c^.tlsCtx := NIL;
  c^.tlsSess := NIL;
  c^.stream := NIL;

  (* 3. Create promise *)
  pst := PromiseCreate(sched, c^.promise, outFuture);
  IF pst # Promise.OK THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  (* 4. Create response *)
  ALLOCATE(c^.resp, TSIZE(Response));
  IF c^.resp = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;
  c^.resp^.statusCode := 0;
  c^.resp^.headerCount := 0;
  c^.resp^.body := NIL;
  c^.resp^.contentLength := -1;

  (* 5. Create receive buffer *)
  bst := Buffers.Create(RecvBufCap, Buffers.Growable, c^.recvBuf);
  IF bst # Buffers.OK THEN
    DEALLOCATE(c^.resp, TSIZE(Response));
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  (* 6. Build request *)
  BuildRequest(c, method, uri);

  (* 7. Create socket *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, c^.sock);
  IF sst # Sockets.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;
  sst := SetNonBlocking(c^.sock, TRUE);

  (* 8. Set up TLS if needed *)
  IF wantTLS THEN
    tst := TLS.ContextCreate(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    IF mSkipVerify THEN
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.NoVerify)
    ELSE
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.VerifyPeer)
    END;
    tst := TLS.SetMinVersion(c^.tlsCtx, TLS.TLS12);
    tst := TLS.LoadSystemRoots(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SessionCreate(lp, sched, c^.tlsCtx, c^.sock,
                              c^.tlsSess);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SetSNI(c^.tlsSess, uri.host)
  END;

  (* 9. Connect *)
  crc := m2_connect_ipv4(c^.sock,
                          ORD(ap^.addrV4[0]),
                          ORD(ap^.addrV4[1]),
                          ORD(ap^.addrV4[2]),
                          ORD(ap^.addrV4[3]),
                          uri.port);
  DEALLOCATE(ap, TSIZE(AddrRec));

  IF crc < 0 THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  IF (crc = 0) AND (NOT wantTLS) THEN
    (* Already connected (non-TLS) *)
    c^.state := StSending;
    est := EventLoop.WatchFd(lp, c^.sock, EvWrite, OnSocketEvent, c)
  ELSE
    (* In progress, or connected but need TLS handshake *)
    est := EventLoop.WatchFd(lp, c^.sock, EvWrite, OnSocketEvent, c)
  END;

  IF est # EventLoop.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  RETURN OK
END DoRequest;

(* ── Public API ────────────────────────────────────────────────── *)

PROCEDURE Get(lp: EventLoop.Loop; sched: Scheduler;
              VAR uri: URIRec;
              VAR outFuture: Future): Status;
VAR method: ARRAY [0..3] OF CHAR;
BEGIN
  method[0] := 'G'; method[1] := 'E'; method[2] := 'T'; method[3] := 0C;
  RETURN DoRequest(lp, sched, uri, method, FALSE, outFuture)
END Get;

PROCEDURE Head(lp: EventLoop.Loop; sched: Scheduler;
               VAR uri: URIRec;
               VAR outFuture: Future): Status;
VAR method: ARRAY [0..4] OF CHAR;
BEGIN
  method[0] := 'H'; method[1] := 'E'; method[2] := 'A';
  method[3] := 'D'; method[4] := 0C;
  RETURN DoRequest(lp, sched, uri, method, TRUE, outFuture)
END Head;

(* ── Request with body (PUT/POST) ─────────────────────────────── *)

PROCEDURE DoRequestWithBody(lp: EventLoop.Loop; sched: Scheduler;
                            VAR uri: URIRec;
                            VAR method: ARRAY OF CHAR;
                            bodyData: ADDRESS; bodyLen: INTEGER;
                            VAR contentType: ARRAY OF CHAR;
                            VAR authorization: ARRAY OF CHAR;
                            VAR outFuture: Future): Status;
VAR
  c: ConnPtr;
  dnsFuture: Future;
  dnsSettled: BOOLEAN;
  dnsResult: Result;
  ap: AddrPtr;
  pst: Promise.Status;
  dst: DNS.Status;
  sst: Sockets.Status;
  bst: Buffers.Status;
  est: EventLoop.Status;
  tst: TLS.Status;
  crc: INTEGER;
  wantTLS: BOOLEAN;
BEGIN
  IF (lp = NIL) OR (sched = NIL) THEN RETURN Invalid END;

  wantTLS := IsHTTPS(uri);

  (* 1. Resolve DNS *)
  dst := DNS.ResolveA(lp, sched, uri.host, uri.port, dnsFuture);
  IF dst # DNS.OK THEN RETURN DNSFailed END;
  pst := GetResultIfSettled(dnsFuture, dnsSettled, dnsResult);
  IF (NOT dnsSettled) OR (NOT dnsResult.isOk) THEN
    RETURN DNSFailed
  END;
  ap := dnsResult.v.ptr;

  (* 2. Allocate connection context *)
  ALLOCATE(c, TSIZE(ConnRec));
  IF c = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    RETURN OutOfMemory
  END;
  c^.state := StConnecting;
  c^.loop := lp;
  c^.sched := sched;
  c^.headOnly := FALSE;
  c^.contentLen := -1;
  c^.bodyRead := 0;
  c^.chunked := FALSE;
  c^.chunkState := ChSize;
  c^.chunkRem := 0;
  c^.reqSent := 0;
  c^.reqBody := bodyData;
  c^.reqBodyLen := bodyLen;
  c^.reqBodySent := 0;
  c^.sock := InvalidSocket;
  c^.recvBuf := NIL;
  c^.resp := NIL;
  c^.useTLS := wantTLS;
  c^.tlsCtx := NIL;
  c^.tlsSess := NIL;
  c^.stream := NIL;

  (* 3. Create promise *)
  pst := PromiseCreate(sched, c^.promise, outFuture);
  IF pst # Promise.OK THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  (* 4. Create response *)
  ALLOCATE(c^.resp, TSIZE(Response));
  IF c^.resp = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;
  c^.resp^.statusCode := 0;
  c^.resp^.headerCount := 0;
  c^.resp^.body := NIL;
  c^.resp^.contentLength := -1;

  (* 5. Create receive buffer *)
  bst := Buffers.Create(RecvBufCap, Buffers.Growable, c^.recvBuf);
  IF bst # Buffers.OK THEN
    DEALLOCATE(c^.resp, TSIZE(Response));
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  (* 6. Build request with body headers *)
  BuildRequestWithBody(c, method, uri, bodyLen,
                       contentType, authorization);

  (* 7. Create socket *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, c^.sock);
  IF sst # Sockets.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;
  sst := SetNonBlocking(c^.sock, TRUE);

  (* 8. Set up TLS if needed *)
  IF wantTLS THEN
    tst := TLS.ContextCreate(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    IF mSkipVerify THEN
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.NoVerify)
    ELSE
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.VerifyPeer)
    END;
    tst := TLS.SetMinVersion(c^.tlsCtx, TLS.TLS12);
    tst := TLS.LoadSystemRoots(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SessionCreate(lp, sched, c^.tlsCtx, c^.sock,
                              c^.tlsSess);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SetSNI(c^.tlsSess, uri.host)
  END;

  (* 9. Connect *)
  crc := m2_connect_ipv4(c^.sock,
                          ORD(ap^.addrV4[0]),
                          ORD(ap^.addrV4[1]),
                          ORD(ap^.addrV4[2]),
                          ORD(ap^.addrV4[3]),
                          uri.port);
  DEALLOCATE(ap, TSIZE(AddrRec));

  IF crc < 0 THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  IF (crc = 0) AND (NOT wantTLS) THEN
    c^.state := StSending;
    est := EventLoop.WatchFd(lp, c^.sock, EvWrite, OnSocketEvent, c)
  ELSE
    est := EventLoop.WatchFd(lp, c^.sock, EvWrite, OnSocketEvent, c)
  END;

  IF est # EventLoop.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  RETURN OK
END DoRequestWithBody;

PROCEDURE Put(lp: EventLoop.Loop; sched: Scheduler;
              VAR uri: URIRec;
              bodyData: ADDRESS; bodyLen: INTEGER;
              VAR contentType: ARRAY OF CHAR;
              VAR authorization: ARRAY OF CHAR;
              VAR outFuture: Future): Status;
VAR method: ARRAY [0..3] OF CHAR;
BEGIN
  method[0] := 'P'; method[1] := 'U'; method[2] := 'T'; method[3] := 0C;
  RETURN DoRequestWithBody(lp, sched, uri, method, bodyData, bodyLen,
                           contentType, authorization, outFuture)
END Put;

PROCEDURE Post(lp: EventLoop.Loop; sched: Scheduler;
               VAR uri: URIRec;
               bodyData: ADDRESS; bodyLen: INTEGER;
               VAR contentType: ARRAY OF CHAR;
               VAR authorization: ARRAY OF CHAR;
               VAR outFuture: Future): Status;
VAR method: ARRAY [0..4] OF CHAR;
BEGIN
  method[0] := 'P'; method[1] := 'O'; method[2] := 'S';
  method[3] := 'T'; method[4] := 0C;
  RETURN DoRequestWithBody(lp, sched, uri, method, bodyData, bodyLen,
                           contentType, authorization, outFuture)
END Post;

PROCEDURE Delete(lp: EventLoop.Loop; sched: Scheduler;
                 VAR uri: URIRec;
                 VAR authorization: ARRAY OF CHAR;
                 VAR outFuture: Future): Status;
VAR method: ARRAY [0..6] OF CHAR;
BEGIN
  method[0] := 'D'; method[1] := 'E'; method[2] := 'L'; method[3] := 'E';
  method[4] := 'T'; method[5] := 'E'; method[6] := 0C;
  RETURN DoRequestWithDelete(lp, sched, uri, method, authorization, outFuture)
END Delete;

PROCEDURE Patch(lp: EventLoop.Loop; sched: Scheduler;
                VAR uri: URIRec;
                bodyData: ADDRESS; bodyLen: INTEGER;
                VAR contentType: ARRAY OF CHAR;
                VAR authorization: ARRAY OF CHAR;
                VAR outFuture: Future): Status;
VAR method: ARRAY [0..5] OF CHAR;
BEGIN
  method[0] := 'P'; method[1] := 'A'; method[2] := 'T';
  method[3] := 'C'; method[4] := 'H'; method[5] := 0C;
  RETURN DoRequestWithBody(lp, sched, uri, method, bodyData, bodyLen,
                           contentType, authorization, outFuture)
END Patch;

(* ── DELETE with auth header ───────────────────────────────────── *)

PROCEDURE BuildRequestWithAuth(c: ConnPtr; VAR method: ARRAY OF CHAR;
                                VAR uri: URIRec;
                                VAR authorization: ARRAY OF CHAR);
VAR
  rpath: ARRAY [0..2047] OF CHAR;
  rpLen: INTEGER;
  ust: URI.Status;
BEGIN
  c^.reqLen := 0;
  AppendStr(c, method);
  AppendCh(c, ' ');

  ust := RequestPath(uri, rpath, rpLen);
  IF rpLen > 0 THEN
    AppendStr(c, rpath)
  ELSE
    AppendCh(c, '/')
  END;

  AppendStr(c, " HTTP/1.1");
  AppendCRLF(c);
  AppendStr(c, "Host: ");
  AppendStr(c, uri.host);
  AppendCRLF(c);
  AppendStr(c, "Connection: close");
  AppendCRLF(c);
  AppendStr(c, "User-Agent: m2http/0.1");
  AppendCRLF(c);
  IF StrLen(authorization) > 0 THEN
    AppendStr(c, "Authorization: ");
    AppendStr(c, authorization);
    AppendCRLF(c)
  END;
  AppendCRLF(c)
END BuildRequestWithAuth;

PROCEDURE DoRequestWithDelete(lp: EventLoop.Loop; sched: Scheduler;
                              VAR uri: URIRec;
                              VAR method: ARRAY OF CHAR;
                              VAR authorization: ARRAY OF CHAR;
                              VAR outFuture: Future): Status;
VAR
  c: ConnPtr;
  dnsFuture: Future;
  dnsSettled: BOOLEAN;
  dnsResult: Result;
  ap: AddrPtr;
  pst: Promise.Status;
  dst: DNS.Status;
  sst: Sockets.Status;
  bst: Buffers.Status;
  est: EventLoop.Status;
  tst: TLS.Status;
  crc: INTEGER;
  wantTLS: BOOLEAN;
BEGIN
  IF (lp = NIL) OR (sched = NIL) THEN RETURN Invalid END;

  wantTLS := IsHTTPS(uri);

  dst := DNS.ResolveA(lp, sched, uri.host, uri.port, dnsFuture);
  IF dst # DNS.OK THEN RETURN DNSFailed END;
  pst := GetResultIfSettled(dnsFuture, dnsSettled, dnsResult);
  IF (NOT dnsSettled) OR (NOT dnsResult.isOk) THEN
    RETURN DNSFailed
  END;
  ap := dnsResult.v.ptr;

  ALLOCATE(c, TSIZE(ConnRec));
  IF c = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    RETURN OutOfMemory
  END;
  c^.state := StConnecting;
  c^.loop := lp;
  c^.sched := sched;
  c^.headOnly := FALSE;
  c^.contentLen := -1;
  c^.bodyRead := 0;
  c^.chunked := FALSE;
  c^.chunkState := ChSize;
  c^.chunkRem := 0;
  c^.reqSent := 0;
  c^.reqBody := NIL;
  c^.reqBodyLen := 0;
  c^.reqBodySent := 0;
  c^.sock := InvalidSocket;
  c^.recvBuf := NIL;
  c^.resp := NIL;
  c^.useTLS := wantTLS;
  c^.tlsCtx := NIL;
  c^.tlsSess := NIL;
  c^.stream := NIL;

  pst := PromiseCreate(sched, c^.promise, outFuture);
  IF pst # Promise.OK THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  ALLOCATE(c^.resp, TSIZE(Response));
  IF c^.resp = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;
  c^.resp^.statusCode := 0;
  c^.resp^.headerCount := 0;
  c^.resp^.body := NIL;
  c^.resp^.contentLength := -1;

  bst := Buffers.Create(RecvBufCap, Buffers.Growable, c^.recvBuf);
  IF bst # Buffers.OK THEN
    DEALLOCATE(c^.resp, TSIZE(Response));
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  BuildRequestWithAuth(c, method, uri, authorization);

  sst := SocketCreate(AF_INET, SOCK_STREAM, c^.sock);
  IF sst # Sockets.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;
  sst := SetNonBlocking(c^.sock, TRUE);

  IF wantTLS THEN
    tst := TLS.ContextCreate(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    IF mSkipVerify THEN
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.NoVerify)
    ELSE
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.VerifyPeer)
    END;
    tst := TLS.SetMinVersion(c^.tlsCtx, TLS.TLS12);
    tst := TLS.LoadSystemRoots(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SessionCreate(lp, sched, c^.tlsCtx, c^.sock,
                              c^.tlsSess);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SetSNI(c^.tlsSess, uri.host)
  END;

  crc := m2_connect_ipv4(c^.sock,
                          ORD(ap^.addrV4[0]),
                          ORD(ap^.addrV4[1]),
                          ORD(ap^.addrV4[2]),
                          ORD(ap^.addrV4[3]),
                          uri.port);
  DEALLOCATE(ap, TSIZE(AddrRec));

  IF crc < 0 THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  est := EventLoop.WatchFd(lp, c^.sock, EvWrite, OnSocketEvent, c);
  IF est # EventLoop.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  RETURN OK
END DoRequestWithDelete;

(* ── Chunked request support ──────────────────────────────────── *)

PROCEDURE IntToHex(val: INTEGER; VAR buf: ARRAY OF CHAR; VAR len: INTEGER);
VAR
  tmp: ARRAY [0..15] OF CHAR;
  i, j, d: INTEGER;
BEGIN
  IF val = 0 THEN
    buf[0] := '0'; len := 1; RETURN
  END;
  i := 0;
  WHILE (val > 0) AND (i < 15) DO
    d := val MOD 16;
    IF d < 10 THEN
      tmp[i] := CHR(ORD('0') + d)
    ELSE
      tmp[i] := CHR(ORD('a') + d - 10)
    END;
    val := val DIV 16;
    INC(i)
  END;
  j := 0;
  WHILE i > 0 DO
    DEC(i);
    IF j <= HIGH(buf) THEN buf[j] := tmp[i]; INC(j) END
  END;
  len := j
END IntToHex;

PROCEDURE BuildChunkedRequest(c: ConnPtr; VAR method: ARRAY OF CHAR;
                               VAR uri: URIRec;
                               VAR contentType: ARRAY OF CHAR;
                               VAR authorization: ARRAY OF CHAR);
VAR
  rpath: ARRAY [0..2047] OF CHAR;
  rpLen: INTEGER;
  ust: URI.Status;
BEGIN
  c^.reqLen := 0;
  AppendStr(c, method);
  AppendCh(c, ' ');

  ust := RequestPath(uri, rpath, rpLen);
  IF rpLen > 0 THEN
    AppendStr(c, rpath)
  ELSE
    AppendCh(c, '/')
  END;

  AppendStr(c, " HTTP/1.1");
  AppendCRLF(c);
  AppendStr(c, "Host: ");
  AppendStr(c, uri.host);
  AppendCRLF(c);
  AppendStr(c, "Connection: close");
  AppendCRLF(c);
  AppendStr(c, "User-Agent: m2http/0.1");
  AppendCRLF(c);
  IF StrLen(contentType) > 0 THEN
    AppendStr(c, "Content-Type: ");
    AppendStr(c, contentType);
    AppendCRLF(c)
  END;
  AppendStr(c, "Transfer-Encoding: chunked");
  AppendCRLF(c);
  IF StrLen(authorization) > 0 THEN
    AppendStr(c, "Authorization: ");
    AppendStr(c, authorization);
    AppendCRLF(c)
  END;
  AppendCRLF(c)
END BuildChunkedRequest;

PROCEDURE PostChunked(lp: EventLoop.Loop; sched: Scheduler;
                      VAR uri: URIRec;
                      chunker: ChunkProc; ctx: ADDRESS;
                      VAR contentType: ARRAY OF CHAR;
                      VAR authorization: ARRAY OF CHAR;
                      VAR outFuture: Future): Status;
VAR
  c: ConnPtr;
  dnsFuture: Future;
  dnsSettled: BOOLEAN;
  dnsResult: Result;
  ap: AddrPtr;
  pst: Promise.Status;
  dst: DNS.Status;
  sst: Sockets.Status;
  bst: Buffers.Status;
  est: EventLoop.Status;
  tst: TLS.Status;
  crc: INTEGER;
  wantTLS: BOOLEAN;
  method: ARRAY [0..4] OF CHAR;
  chunkBuf: ARRAY [0..4095] OF CHAR;
  hexBuf: ARRAY [0..15] OF CHAR;
  hexLen, chunkLen, total, i: INTEGER;
BEGIN
  IF (lp = NIL) OR (sched = NIL) THEN RETURN Invalid END;

  wantTLS := IsHTTPS(uri);

  dst := DNS.ResolveA(lp, sched, uri.host, uri.port, dnsFuture);
  IF dst # DNS.OK THEN RETURN DNSFailed END;
  pst := GetResultIfSettled(dnsFuture, dnsSettled, dnsResult);
  IF (NOT dnsSettled) OR (NOT dnsResult.isOk) THEN
    RETURN DNSFailed
  END;
  ap := dnsResult.v.ptr;

  ALLOCATE(c, TSIZE(ConnRec));
  IF c = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    RETURN OutOfMemory
  END;
  c^.state := StConnecting;
  c^.loop := lp;
  c^.sched := sched;
  c^.headOnly := FALSE;
  c^.contentLen := -1;
  c^.bodyRead := 0;
  c^.chunked := FALSE;
  c^.chunkState := ChSize;
  c^.chunkRem := 0;
  c^.reqSent := 0;
  c^.reqBody := NIL;
  c^.reqBodyLen := 0;
  c^.reqBodySent := 0;
  c^.sock := InvalidSocket;
  c^.recvBuf := NIL;
  c^.resp := NIL;
  c^.useTLS := wantTLS;
  c^.tlsCtx := NIL;
  c^.tlsSess := NIL;
  c^.stream := NIL;

  pst := PromiseCreate(sched, c^.promise, outFuture);
  IF pst # Promise.OK THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  ALLOCATE(c^.resp, TSIZE(Response));
  IF c^.resp = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;
  c^.resp^.statusCode := 0;
  c^.resp^.headerCount := 0;
  c^.resp^.body := NIL;
  c^.resp^.contentLength := -1;

  bst := Buffers.Create(RecvBufCap, Buffers.Growable, c^.recvBuf);
  IF bst # Buffers.OK THEN
    DEALLOCATE(c^.resp, TSIZE(Response));
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(ConnRec));
    RETURN OutOfMemory
  END;

  method[0] := 'P'; method[1] := 'O'; method[2] := 'S';
  method[3] := 'T'; method[4] := 0C;
  BuildChunkedRequest(c, method, uri, contentType, authorization);

  (* Collect chunked body into reqBody buffer *)
  bst := Buffers.Create(RecvBufCap, Buffers.Growable, c^.resp^.body);
  (* Reuse a temporary growable buffer for building the chunked body *)
  total := 0;
  LOOP
    chunkLen := chunker(ctx, ADR(chunkBuf), 4096);
    IF chunkLen <= 0 THEN EXIT END;
    (* Append hex-length + CRLF + data + CRLF to request body area *)
    IntToHex(chunkLen, hexBuf, hexLen);
    FOR i := 0 TO hexLen - 1 DO
      bst := Buffers.AppendByte(c^.resp^.body, hexBuf[i])
    END;
    bst := Buffers.AppendByte(c^.resp^.body, CHR(13));
    bst := Buffers.AppendByte(c^.resp^.body, CHR(10));
    FOR i := 0 TO chunkLen - 1 DO
      bst := Buffers.AppendByte(c^.resp^.body, chunkBuf[i])
    END;
    bst := Buffers.AppendByte(c^.resp^.body, CHR(13));
    bst := Buffers.AppendByte(c^.resp^.body, CHR(10));
    total := total + chunkLen
  END;
  (* Append final chunk: 0\r\n\r\n *)
  bst := Buffers.AppendByte(c^.resp^.body, '0');
  bst := Buffers.AppendByte(c^.resp^.body, CHR(13));
  bst := Buffers.AppendByte(c^.resp^.body, CHR(10));
  bst := Buffers.AppendByte(c^.resp^.body, CHR(13));
  bst := Buffers.AppendByte(c^.resp^.body, CHR(10));

  (* Move chunked body to reqBody *)
  c^.reqBody := Buffers.SlicePtr(c^.resp^.body);
  c^.reqBodyLen := Buffers.Length(c^.resp^.body);
  c^.reqBodySent := 0;
  (* Keep resp^.body buffer alive — it holds the chunked data.
     The body will be replaced after response is received. *)

  sst := SocketCreate(AF_INET, SOCK_STREAM, c^.sock);
  IF sst # Sockets.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;
  sst := SetNonBlocking(c^.sock, TRUE);

  IF wantTLS THEN
    tst := TLS.ContextCreate(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    IF mSkipVerify THEN
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.NoVerify)
    ELSE
      tst := TLS.SetVerifyMode(c^.tlsCtx, TLS.VerifyPeer)
    END;
    tst := TLS.SetMinVersion(c^.tlsCtx, TLS.TLS12);
    tst := TLS.LoadSystemRoots(c^.tlsCtx);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SessionCreate(lp, sched, c^.tlsCtx, c^.sock,
                              c^.tlsSess);
    IF tst # TLS.OK THEN
      FailConn(c, 6);
      RETURN TLSFailed
    END;
    tst := TLS.SetSNI(c^.tlsSess, uri.host)
  END;

  crc := m2_connect_ipv4(c^.sock,
                          ORD(ap^.addrV4[0]),
                          ORD(ap^.addrV4[1]),
                          ORD(ap^.addrV4[2]),
                          ORD(ap^.addrV4[3]),
                          uri.port);
  DEALLOCATE(ap, TSIZE(AddrRec));

  IF crc < 0 THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  est := EventLoop.WatchFd(lp, c^.sock, EvWrite, OnSocketEvent, c);
  IF est # EventLoop.OK THEN
    FailConn(c, 1);
    RETURN ConnectFailed
  END;

  RETURN OK
END PostChunked;

(* ── Response helpers ──────────────────────────────────────────── *)

PROCEDURE FindHeader(resp: ResponsePtr;
                     VAR name: ARRAY OF CHAR;
                     VAR out: ARRAY OF CHAR): BOOLEAN;
VAR i, j, nLen: INTEGER;
BEGIN
  IF resp = NIL THEN RETURN FALSE END;
  nLen := StrLen(name);
  FOR i := 0 TO resp^.headerCount - 1 DO
    IF StrEqCI(resp^.headers[i].name, name,
               resp^.headers[i].nameLen, nLen) THEN
      FOR j := 0 TO resp^.headers[i].valueLen - 1 DO
        IF j <= HIGH(out) THEN
          out[j] := resp^.headers[i].value[j]
        END
      END;
      IF resp^.headers[i].valueLen <= HIGH(out) THEN
        out[resp^.headers[i].valueLen] := 0C
      END;
      RETURN TRUE
    END
  END;
  RETURN FALSE
END FindHeader;

PROCEDURE FreeResponse(VAR resp: ResponsePtr);
VAR bst: Buffers.Status;
BEGIN
  IF resp = NIL THEN RETURN END;
  IF resp^.body # NIL THEN
    bst := Buffers.Destroy(resp^.body);
    resp^.body := NIL
  END;
  DEALLOCATE(resp, TSIZE(Response));
  resp := NIL
END FreeResponse;

PROCEDURE SetSkipVerify(skip: BOOLEAN);
BEGIN
  mSkipVerify := skip
END SetSkipVerify;

BEGIN
  mSkipVerify := FALSE
END HTTPClient.
