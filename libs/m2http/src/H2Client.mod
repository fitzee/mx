IMPLEMENTATION MODULE H2Client;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Value,
                    PromiseCreate, PromiseRelease, Resolve, Reject,
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
IMPORT ByteBuf;
FROM ByteBuf IMPORT BytesView, Buf;
FROM Http2Types IMPORT FrameHeader, HeaderEntry,
                       FrameHeaders, FrameData, FrameSettings,
                       FramePing, FrameGoaway, FrameWindowUpdate,
                       FlagEndStream, FlagAck, FlagEndHeaders,
                       DefaultMaxFrameSize, DefaultHeaderTableSize,
                       SetMaxFrameSize;
IMPORT Http2Hpack;
FROM Http2Hpack IMPORT DynTable;
IMPORT HTTPClient;
FROM HTTPClient IMPORT Response, ResponsePtr, Header, Status,
                       MaxHeaders, MaxHeaderName, MaxHeaderVal;

(* ── Connection states ──────────────────────────────────── *)

CONST
  StConnecting   = 0;
  StHandshaking  = 1;
  StSendPreface  = 2;
  StWaitSettings = 3;
  StSendRequest  = 4;
  StRecvResponse = 5;
  StDone         = 6;
  StError        = 7;

  RecvBufCap = 16384;

TYPE
  H2ConnRec = RECORD
    state:      INTEGER;
    sock:       Socket;
    promise:    Promise.Promise;
    loop:       EventLoop.Loop;
    sched:      Scheduler;
    resp:       ResponsePtr;
    tlsCtx:     TLS.TLSContext;
    tlsSess:    TLS.TLSSession;
    stream:     Stream.Stream;
    outBuf:     Buf;
    dynEnc:     DynTable;
    dynDec:     DynTable;
    remoteMaxFrame: CARDINAL;
    streamId:   CARDINAL;
    recvBuf:    ARRAY [0..RecvBufCap-1] OF CHAR;
    recvLen:    INTEGER;
    sendPtr:    ADDRESS;
    sendTotal:  INTEGER;
    sendPos:    INTEGER;
    reqBody:    ADDRESS;
    reqBodyLen: INTEGER;
    gotSettings:  BOOLEAN;
    gotHeaders:   BOOLEAN;
    gotEndStream: BOOLEAN;
    goawayRecv:   BOOLEAN;
    method:     ARRAY [0..7] OF CHAR;
    hasBody:    BOOLEAN;
    host:       ARRAY [0..255] OF CHAR;
    authority:  ARRAY [0..271] OF CHAR;
    path:       ARRAY [0..2047] OF CHAR;
    pathLen:    INTEGER;
    contentType:   ARRAY [0..127] OF CHAR;
    authorization: ARRAY [0..511] OF CHAR;
    bodyLen:    INTEGER;
    hdrBuf:     Buf;
  END;

  H2ConnPtr = POINTER TO H2ConnRec;

VAR
  mSkipVerify: BOOLEAN;
  mALPN: ARRAY [0..2] OF CHAR;

(* ── String helpers ──────────────────────────────────────── *)

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): INTEGER;
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
  RETURN i
END StrLen;

PROCEDURE StrCopy(VAR src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(src)) AND (i <= HIGH(dst)) AND (src[i] # 0C) DO
    dst[i] := src[i];
    INC(i)
  END;
  IF i <= HIGH(dst) THEN dst[i] := 0C END
END StrCopy;

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
  j := 0;
  WHILE i > 0 DO
    DEC(i);
    IF j <= HIGH(buf) THEN
      buf[j] := tmp[i]; INC(j)
    END
  END;
  IF j <= HIGH(buf) THEN buf[j] := 0C END
END IntToStr;

PROCEDURE HasFlag(flags, flag: CARDINAL): BOOLEAN;
BEGIN
  RETURN (flags DIV flag) MOD 2 = 1
END HasFlag;

(* ── Inline frame I/O (replaces Http2Frame/Http2Conn) ───── *)

PROCEDURE WriteFrameHeader(VAR b: Buf;
                            length, ftype, flags, streamId: CARDINAL);
BEGIN
  ByteBuf.AppendByte(b, length DIV 65536 MOD 256);
  ByteBuf.AppendByte(b, length DIV 256 MOD 256);
  ByteBuf.AppendByte(b, length MOD 256);
  ByteBuf.AppendByte(b, ftype MOD 256);
  ByteBuf.AppendByte(b, flags MOD 256);
  ByteBuf.AppendByte(b, streamId DIV 16777216 MOD 128);
  ByteBuf.AppendByte(b, streamId DIV 65536 MOD 256);
  ByteBuf.AppendByte(b, streamId DIV 256 MOD 256);
  ByteBuf.AppendByte(b, streamId MOD 256)
END WriteFrameHeader;

PROCEDURE ParseFrameHeader(VAR buf: ARRAY OF CHAR; pos: INTEGER;
                            VAR hdr: FrameHeader; VAR ok: BOOLEAN);
VAR b0, b1, b2, b3, b4, b5, b6, b7, b8: CARDINAL;
BEGIN
  b0 := ORD(buf[pos]) MOD 256;
  b1 := ORD(buf[pos+1]) MOD 256;
  b2 := ORD(buf[pos+2]) MOD 256;
  b3 := ORD(buf[pos+3]) MOD 256;
  b4 := ORD(buf[pos+4]) MOD 256;
  b5 := ORD(buf[pos+5]) MOD 256;
  b6 := ORD(buf[pos+6]) MOD 256;
  b7 := ORD(buf[pos+7]) MOD 256;
  b8 := ORD(buf[pos+8]) MOD 256;
  hdr.length := b0 * 65536 + b1 * 256 + b2;
  hdr.ftype := b3;
  hdr.flags := b4;
  hdr.streamId := (b5 MOD 128) * 16777216 + b6 * 65536 + b7 * 256 + b8;
  ok := TRUE
END ParseFrameHeader;

PROCEDURE WritePreface(VAR b: Buf);
VAR preface: ARRAY [0..23] OF CHAR;
    dummy: BOOLEAN;
BEGIN
  preface := "PRI * HTTP/2.0";
  preface[14] := CHR(13); preface[15] := CHR(10);
  preface[16] := CHR(13); preface[17] := CHR(10);
  preface[18] := 'S'; preface[19] := 'M';
  preface[20] := CHR(13); preface[21] := CHR(10);
  preface[22] := CHR(13); preface[23] := CHR(10);
  dummy := ByteBuf.AppendChars(b, preface, 24)
END WritePreface;

(* ── TLS-aware I/O helpers ───────────────────────────────── *)

PROCEDURE DoSend(c: H2ConnPtr; buf: ADDRESS; len: INTEGER): INTEGER;
VAR n: INTEGER; st: Stream.Status;
BEGIN
  st := Stream.TryWrite(c^.stream, buf, len, n);
  IF st = Stream.OK THEN RETURN n
  ELSIF st = Stream.WouldBlock THEN RETURN -2
  ELSE RETURN -1
  END
END DoSend;

PROCEDURE DoRecv(c: H2ConnPtr; buf: ADDRESS; max: INTEGER): INTEGER;
VAR n: INTEGER; st: Stream.Status;
BEGIN
  st := Stream.TryRead(c^.stream, buf, max, n);
  IF st = Stream.OK THEN RETURN n
  ELSIF st = Stream.StreamClosed THEN RETURN 0
  ELSIF st = Stream.WouldBlock THEN RETURN -2
  ELSE RETURN -1
  END
END DoRecv;

(* ── Cleanup ─────────────────────────────────────────────── *)

PROCEDURE CleanupConn(c: H2ConnPtr);
VAR est: EventLoop.Status; sst: Sockets.Status;
    tst: TLS.Status; stst: Stream.Status;
BEGIN
  IF c^.stream # NIL THEN
    stst := Stream.Destroy(c^.stream);
    c^.stream := NIL
  END;
  IF c^.tlsSess # NIL THEN
    tst := TLS.Shutdown(c^.tlsSess);
    tst := TLS.SessionDestroy(c^.tlsSess);
    c^.tlsSess := NIL
  END;
  IF c^.tlsCtx # NIL THEN
    tst := TLS.ContextDestroy(c^.tlsCtx);
    c^.tlsCtx := NIL
  END;
  IF c^.sock # InvalidSocket THEN
    est := EventLoop.UnwatchFd(c^.loop, c^.sock);
    sst := Sockets.CloseSocket(c^.sock);
    c^.sock := InvalidSocket
  END;
  ByteBuf.Free(c^.outBuf);
  ByteBuf.Free(c^.hdrBuf)
END CleanupConn;

PROCEDURE FailConn(c: H2ConnPtr; code: INTEGER);
VAR e: Promise.Error; dummy: Promise.Status; bst: Buffers.Status;
BEGIN
  CleanupConn(c);
  e.code := code;
  e.ptr := NIL;
  dummy := Reject(c^.promise, e);
  PromiseRelease(c^.promise); c^.promise := NIL;
  c^.state := StError;
  IF c^.resp # NIL THEN
    IF c^.resp^.body # NIL THEN
      bst := Buffers.Destroy(c^.resp^.body);
      c^.resp^.body := NIL
    END;
    DEALLOCATE(c^.resp, TSIZE(Response));
    c^.resp := NIL
  END;
  DEALLOCATE(c, TSIZE(H2ConnRec))
END FailConn;

PROCEDURE SucceedConn(c: H2ConnPtr);
VAR v: Value; dummy: Promise.Status; bst: Buffers.Status;
BEGIN
  (* Ensure body buffer exists even if no DATA frames received *)
  IF c^.resp^.body = NIL THEN
    bst := Buffers.Create(Buffers.DefaultCap, Buffers.Growable,
                          c^.resp^.body)
  END;
  CleanupConn(c);
  v.tag := 0;
  v.ptr := c^.resp;
  dummy := Resolve(c^.promise, v);
  PromiseRelease(c^.promise); c^.promise := NIL;
  c^.state := StDone;
  c^.resp := NIL;
  DEALLOCATE(c, TSIZE(H2ConnRec))
END SucceedConn;

(* ── HPACK header helpers ────────────────────────────────── *)

PROCEDURE SetHdr(VAR e: HeaderEntry;
                 VAR name: ARRAY OF CHAR; nLen: INTEGER;
                 VAR value: ARRAY OF CHAR; vLen: INTEGER);
VAR i: INTEGER;
BEGIN
  FOR i := 0 TO nLen - 1 DO
    IF i < 128 THEN e.name[i] := name[i] END
  END;
  IF nLen < 128 THEN e.name[nLen] := 0C END;
  e.nameLen := nLen;
  FOR i := 0 TO vLen - 1 DO
    IF i < 4096 THEN e.value[i] := value[i] END
  END;
  IF vLen < 4096 THEN e.value[vLen] := 0C END;
  e.valLen := vLen
END SetHdr;

PROCEDURE SetHdrS(VAR e: HeaderEntry;
                  VAR name: ARRAY OF CHAR;
                  VAR value: ARRAY OF CHAR);
BEGIN
  SetHdr(e, name, StrLen(name), value, StrLen(value))
END SetHdrS;

(* ── Build HPACK-encoded request headers ─────────────────── *)

PROCEDURE BuildRequestHeaders(c: H2ConnPtr);
VAR
  headers: ARRAY [0..9] OF HeaderEntry;
  numHdrs: CARDINAL;
  nMethod, nPath, nScheme, nAuth: ARRAY [0..15] OF CHAR;
  vScheme: ARRAY [0..7] OF CHAR;
  nCT, nCL, nAZ, nUA: ARRAY [0..15] OF CHAR;
  vUA: ARRAY [0..15] OF CHAR;
  clBuf: ARRAY [0..15] OF CHAR;
  slash: ARRAY [0..0] OF CHAR;
  pLen: INTEGER;
BEGIN
  numHdrs := 0;

  nMethod := ":method";
  SetHdr(headers[numHdrs], nMethod, 7, c^.method, StrLen(c^.method));
  INC(numHdrs);

  nPath := ":path";
  pLen := c^.pathLen;
  IF pLen > 0 THEN
    SetHdr(headers[numHdrs], nPath, 5, c^.path, pLen)
  ELSE
    slash[0] := '/';
    SetHdr(headers[numHdrs], nPath, 5, slash, 1)
  END;
  INC(numHdrs);

  nScheme := ":scheme";
  vScheme := "https";
  SetHdrS(headers[numHdrs], nScheme, vScheme);
  INC(numHdrs);

  nAuth := ":authority";
  SetHdr(headers[numHdrs], nAuth, 10, c^.authority, StrLen(c^.authority));
  INC(numHdrs);

  IF c^.hasBody THEN
    IF StrLen(c^.contentType) > 0 THEN
      nCT := "content-type";
      SetHdr(headers[numHdrs], nCT, 12,
             c^.contentType, StrLen(c^.contentType));
      INC(numHdrs)
    END;

    nCL := "content-length";
    IntToStr(c^.bodyLen, clBuf);
    SetHdr(headers[numHdrs], nCL, 14, clBuf, StrLen(clBuf));
    INC(numHdrs)
  END;

  (* Authorization header — sent for any method that provides it *)
  IF StrLen(c^.authorization) > 0 THEN
    nAZ := "authorization";
    SetHdr(headers[numHdrs], nAZ, 13,
           c^.authorization, StrLen(c^.authorization));
    INC(numHdrs)
  END;

  nUA := "user-agent";
  vUA := "m2http/0.1";
  SetHdrS(headers[numHdrs], nUA, vUA);
  INC(numHdrs);

  ByteBuf.Clear(c^.hdrBuf);
  Http2Hpack.EncodeHeaderBlock(c^.hdrBuf, c^.dynEnc,
                                headers, numHdrs)
END BuildRequestHeaders;

(* ── Build full request into outBuf ──────────────────────── *)

PROCEDURE BuildRequest(c: H2ConnPtr);
VAR
  endStream: BOOLEAN;
  hdrView, bodyView: BytesView;
  maxFrame, remaining, chunk, flags: CARDINAL;
  offset: INTEGER;
BEGIN
  c^.streamId := 1;  (* first client-initiated stream *)

  BuildRequestHeaders(c);

  endStream := NOT c^.hasBody;
  hdrView := ByteBuf.AsView(c^.hdrBuf);

  (* HEADERS frame *)
  flags := FlagEndHeaders;
  IF endStream THEN flags := flags + FlagEndStream END;
  WriteFrameHeader(c^.outBuf, hdrView.len, FrameHeaders,
                   flags, c^.streamId);
  ByteBuf.AppendView(c^.outBuf, hdrView);

  (* For body-bearing methods: DATA frame(s) *)
  IF c^.hasBody THEN
    IF c^.bodyLen > 0 THEN
      maxFrame := c^.remoteMaxFrame;
      remaining := c^.bodyLen;
      offset := 0;
      WHILE remaining > 0 DO
        IF remaining > maxFrame THEN
          chunk := maxFrame
        ELSE
          chunk := remaining
        END;
        IF remaining - chunk = 0 THEN
          flags := FlagEndStream
        ELSE
          flags := 0
        END;
        WriteFrameHeader(c^.outBuf, chunk, FrameData,
                         flags, c^.streamId);
        bodyView.base := VAL(ADDRESS,
          LONGCARD(c^.reqBody) + LONGCARD(offset));
        bodyView.len := chunk;
        ByteBuf.AppendView(c^.outBuf, bodyView);
        offset := offset + VAL(INTEGER, chunk);
        remaining := remaining - chunk
      END
    ELSE
      (* Empty body: DATA with END_STREAM *)
      WriteFrameHeader(c^.outBuf, 0, FrameData,
                       FlagEndStream, c^.streamId)
    END
  END
END BuildRequest;

(* ── Process received frames ─────────────────────────────── *)

PROCEDURE ProcessRecvFrames(c: H2ConnPtr);
VAR
  pos, avail, remaining, i, j: INTEGER;
  nLen, vLen, statusVal, hc: INTEGER;
  hdr: FrameHeader;
  payload: BytesView;
  ok: BOOLEAN;
  h2Headers: ARRAY [0..63] OF HeaderEntry;
  numH2Hdrs: CARDINAL;
  bst: Buffers.Status;
  spos, sid, sval: CARDINAL;
BEGIN
  pos := 0;
  avail := c^.recvLen;

  LOOP
    IF avail - pos < 9 THEN EXIT END;
    (* Parse 9-byte frame header *)
    ParseFrameHeader(c^.recvBuf, pos, hdr, ok);
    IF NOT ok THEN
      FailConn(c, 5);
      RETURN
    END;

    IF avail - pos < 9 + VAL(INTEGER, hdr.length) THEN
      EXIT  (* incomplete frame, need more data *)
    END;

    (* Extract payload view *)
    payload.base := ADR(c^.recvBuf[pos + 9]);
    payload.len := hdr.length;

    (* ── Inline control frame handling ── *)

    IF hdr.ftype = FrameSettings THEN
      IF NOT HasFlag(hdr.flags, FlagAck) THEN
        (* Parse settings: 6-byte pairs (16-bit id + 32-bit value) *)
        spos := 0;
        WHILE spos + 6 <= hdr.length DO
          sid := ByteBuf.ViewGetByte(payload, spos) * 256 +
                 ByteBuf.ViewGetByte(payload, spos + 1);
          sval := ByteBuf.ViewGetByte(payload, spos + 2) * 16777216 +
                  ByteBuf.ViewGetByte(payload, spos + 3) * 65536 +
                  ByteBuf.ViewGetByte(payload, spos + 4) * 256 +
                  ByteBuf.ViewGetByte(payload, spos + 5);
          IF sid = SetMaxFrameSize THEN
            c^.remoteMaxFrame := sval
          END;
          spos := spos + 6
        END;
        (* Send SETTINGS ACK *)
        WriteFrameHeader(c^.outBuf, 0, FrameSettings, FlagAck, 0);
        c^.gotSettings := TRUE
      END

    ELSIF hdr.ftype = FramePing THEN
      IF NOT HasFlag(hdr.flags, FlagAck) THEN
        (* Echo PING payload back with ACK flag *)
        WriteFrameHeader(c^.outBuf, 8, FramePing, FlagAck, 0);
        ByteBuf.AppendView(c^.outBuf, payload)
      END

    ELSIF hdr.ftype = FrameGoaway THEN
      c^.goawayRecv := TRUE
    END;

    (* WINDOW_UPDATE and other control frames — ignored *)

    IF c^.goawayRecv THEN
      FailConn(c, 3);
      RETURN
    END;

    (* Handle HEADERS frame on our stream *)
    IF (hdr.ftype = FrameHeaders) AND
       (hdr.streamId = c^.streamId) THEN
      numH2Hdrs := 0;
      Http2Hpack.DecodeHeaderBlock(payload, c^.dynDec,
                                    h2Headers, 64,
                                    numH2Hdrs, ok);
      IF ok THEN
        FOR i := 0 TO VAL(INTEGER, numH2Hdrs) - 1 DO
          IF (h2Headers[i].nameLen = 7) AND
             (h2Headers[i].name[0] = ':') AND
             (h2Headers[i].name[1] = 's') AND
             (h2Headers[i].name[2] = 't') AND
             (h2Headers[i].name[3] = 'a') AND
             (h2Headers[i].name[4] = 't') AND
             (h2Headers[i].name[5] = 'u') AND
             (h2Headers[i].name[6] = 's') THEN
            statusVal := 0;
            FOR j := 0 TO VAL(INTEGER, h2Headers[i].valLen) - 1 DO
              IF (h2Headers[i].value[j] >= '0') AND
                 (h2Headers[i].value[j] <= '9') THEN
                statusVal := statusVal * 10 +
                  (ORD(h2Headers[i].value[j]) - ORD('0'))
              END
            END;
            c^.resp^.statusCode := statusVal
          ELSIF h2Headers[i].name[0] # ':' THEN
            hc := c^.resp^.headerCount;
            IF hc < MaxHeaders THEN
              nLen := VAL(INTEGER, h2Headers[i].nameLen);
              IF nLen >= MaxHeaderName THEN
                nLen := MaxHeaderName - 1
              END;
              FOR j := 0 TO nLen - 1 DO
                c^.resp^.headers[hc].name[j] := h2Headers[i].name[j]
              END;
              c^.resp^.headers[hc].name[nLen] := 0C;
              c^.resp^.headers[hc].nameLen := nLen;

              vLen := VAL(INTEGER, h2Headers[i].valLen);
              IF vLen >= MaxHeaderVal THEN
                vLen := MaxHeaderVal - 1
              END;
              FOR j := 0 TO vLen - 1 DO
                c^.resp^.headers[hc].value[j] := h2Headers[i].value[j]
              END;
              c^.resp^.headers[hc].value[vLen] := 0C;
              c^.resp^.headers[hc].valueLen := vLen;
              INC(c^.resp^.headerCount)
            END
          END
        END;
        c^.gotHeaders := TRUE
      END
    END;

    (* Handle DATA frame on our stream *)
    IF (hdr.ftype = FrameData) AND
       (hdr.streamId = c^.streamId) AND
       (hdr.length > 0) THEN
      IF c^.resp^.body = NIL THEN
        bst := Buffers.Create(Buffers.DefaultCap, Buffers.Growable,
                              c^.resp^.body)
      END;
      IF c^.resp^.body # NIL THEN
        FOR i := 0 TO VAL(INTEGER, hdr.length) - 1 DO
          bst := Buffers.AppendByte(c^.resp^.body,
                   CHR(ByteBuf.ViewGetByte(payload, VAL(CARDINAL, i))))
        END
      END
    END;

    (* Check END_STREAM on our stream *)
    IF (hdr.streamId = c^.streamId) AND
       HasFlag(hdr.flags, FlagEndStream) THEN
      c^.gotEndStream := TRUE
    END;

    pos := pos + 9 + VAL(INTEGER, hdr.length)
  END;

  (* Compact buffer: shift unprocessed bytes to front *)
  IF pos > 0 THEN
    remaining := avail - pos;
    FOR i := 0 TO remaining - 1 DO
      c^.recvBuf[i] := c^.recvBuf[pos + i]
    END;
    c^.recvLen := remaining
  END
END ProcessRecvFrames;

(* ── TLS handshake helper ────────────────────────────────── *)

PROCEDURE DoTLSHandshake(c: H2ConnPtr);
VAR hst: TLS.Status; est: EventLoop.Status; sst: Stream.Status;
    view: BytesView;
BEGIN
  hst := TLS.Handshake(c^.tlsSess);
  IF hst = TLS.OK THEN
    (* Handshake complete — create TLS stream *)
    sst := Stream.CreateTLS(c^.loop, c^.sched, c^.sock,
                             c^.tlsCtx, c^.tlsSess, c^.stream);
    c^.tlsCtx := NIL;
    c^.tlsSess := NIL;
    (* Write H2 connection preface + empty SETTINGS *)
    WritePreface(c^.outBuf);
    WriteFrameHeader(c^.outBuf, 0, FrameSettings, 0, 0);
    view := ByteBuf.AsView(c^.outBuf);
    c^.sendPtr := view.base;
    c^.sendTotal := VAL(INTEGER, view.len);
    c^.sendPos := 0;
    c^.state := StSendPreface;
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

(* ── Send helper ─────────────────────────────────────────── *)

PROCEDURE FlushSend(c: H2ConnPtr): BOOLEAN;
VAR n, remaining: INTEGER;
BEGIN
  remaining := c^.sendTotal - c^.sendPos;
  IF remaining <= 0 THEN RETURN TRUE END;
  n := DoSend(c,
    VAL(ADDRESS, LONGCARD(c^.sendPtr) + LONGCARD(c^.sendPos)),
    remaining);
  IF n > 0 THEN
    c^.sendPos := c^.sendPos + n;
    RETURN c^.sendPos >= c^.sendTotal
  ELSIF n = -2 THEN
    RETURN FALSE
  ELSE
    FailConn(c, 2);
    RETURN FALSE
  END
END FlushSend;

(* ── Socket event handler ────────────────────────────────── *)

PROCEDURE OnSocketEvent(fd, events: INTEGER; user: ADDRESS);
VAR
  c: H2ConnPtr;
  n, sockErr: INTEGER;
  est: EventLoop.Status;
  view: BytesView;
BEGIN
  c := user;

  CASE c^.state OF

    StConnecting:
      sockErr := m2_getsockopt_error(fd);
      IF sockErr # 0 THEN
        FailConn(c, 1);
        RETURN
      END;
      DoTLSHandshake(c) |

    StHandshaking:
      DoTLSHandshake(c) |

    StSendPreface:
      IF FlushSend(c) THEN
        ByteBuf.Clear(c^.outBuf);
        c^.state := StWaitSettings;
        est := EventLoop.ModifyFd(c^.loop, c^.sock, EvRead)
      END |

    StWaitSettings:
      n := DoRecv(c, ADR(c^.recvBuf[c^.recvLen]),
                   RecvBufCap - c^.recvLen);
      IF n > 0 THEN
        c^.recvLen := c^.recvLen + n;
        ProcessRecvFrames(c);
        IF c^.state = StError THEN RETURN END;

        IF c^.gotSettings THEN
          (* outBuf already has SETTINGS ACK from ProcessRecvFrames;
             BuildRequest appends HEADERS + DATA *)
          BuildRequest(c);
          IF c^.state = StError THEN RETURN END;
          view := ByteBuf.AsView(c^.outBuf);
          c^.sendPtr := view.base;
          c^.sendTotal := VAL(INTEGER, view.len);
          c^.sendPos := 0;
          c^.state := StSendRequest;
          est := EventLoop.ModifyFd(c^.loop, c^.sock, EvWrite)
        END
      ELSIF n = 0 THEN
        FailConn(c, 3)
      ELSIF n = -2 THEN
        RETURN
      ELSE
        FailConn(c, 3)
      END |

    StSendRequest:
      IF FlushSend(c) THEN
        ByteBuf.Clear(c^.outBuf);
        c^.state := StRecvResponse;
        est := EventLoop.ModifyFd(c^.loop, c^.sock, EvRead)
      END |

    StRecvResponse:
      n := DoRecv(c, ADR(c^.recvBuf[c^.recvLen]),
                   RecvBufCap - c^.recvLen);
      IF n > 0 THEN
        c^.recvLen := c^.recvLen + n;
        ProcessRecvFrames(c);
        IF c^.state = StError THEN RETURN END;

        (* Flush any control frame responses (PING ACK, etc.) *)
        view := ByteBuf.AsView(c^.outBuf);
        IF view.len > 0 THEN
          n := DoSend(c, view.base, VAL(INTEGER, view.len));
          ByteBuf.Clear(c^.outBuf)
        END;

        IF c^.gotEndStream THEN
          SucceedConn(c)
        END
      ELSIF n = 0 THEN
        IF c^.gotHeaders THEN
          SucceedConn(c)
        ELSE
          FailConn(c, 3)
        END
      ELSIF n = -2 THEN
        RETURN
      ELSE
        FailConn(c, 3)
      END

  ELSE
    (* StDone, StError — ignore *)
  END
END OnSocketEvent;

(* ── Build authority string ──────────────────────────────── *)

PROCEDURE BuildAuthority(c: H2ConnPtr; VAR uri: URIRec);
VAR
  portBuf: ARRAY [0..7] OF CHAR;
  i, j: INTEGER;
BEGIN
  StrCopy(uri.host, c^.authority);
  IF uri.port # 443 THEN
    i := StrLen(c^.authority);
    IF i < HIGH(c^.authority) THEN
      c^.authority[i] := ':';
      INC(i);
      IntToStr(uri.port, portBuf);
      j := 0;
      WHILE (j <= HIGH(portBuf)) AND (portBuf[j] # 0C) AND
            (i <= HIGH(c^.authority)) DO
        c^.authority[i] := portBuf[j];
        INC(i); INC(j)
      END;
      IF i <= HIGH(c^.authority) THEN c^.authority[i] := 0C END
    END
  END
END BuildAuthority;

(* ── Core connection setup ───────────────────────────────── *)

PROCEDURE DoConnect(lp: EventLoop.Loop; sched: Scheduler;
                    VAR uri: URIRec;
                    VAR method: ARRAY OF CHAR;
                    withBody: BOOLEAN;
                    bodyData: ADDRESS; bodyLen: INTEGER;
                    VAR contentType: ARRAY OF CHAR;
                    VAR authorization: ARRAY OF CHAR;
                    VAR outFuture: Future): Status;
VAR
  c: H2ConnPtr;
  dnsFuture: Future;
  dnsSettled: BOOLEAN;
  dnsResult: Result;
  ap: AddrPtr;
  pst: Promise.Status;
  dst: DNS.Status;
  sst: Sockets.Status;
  est: EventLoop.Status;
  tst: TLS.Status;
  ust: URI.Status;
  crc: INTEGER;
BEGIN
  IF (lp = NIL) OR (sched = NIL) THEN RETURN HTTPClient.Invalid END;

  (* 1. Resolve DNS *)
  dst := DNS.ResolveA(lp, sched, uri.host, uri.port, dnsFuture);
  IF dst # DNS.OK THEN RETURN HTTPClient.DNSFailed END;
  pst := GetResultIfSettled(dnsFuture, dnsSettled, dnsResult);
  IF (NOT dnsSettled) OR (NOT dnsResult.isOk) THEN
    RETURN HTTPClient.DNSFailed
  END;
  ap := dnsResult.v.ptr;

  (* 2. Allocate connection context *)
  ALLOCATE(c, TSIZE(H2ConnRec));
  IF c = NIL THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    RETURN HTTPClient.OutOfMemory
  END;

  c^.state := StConnecting;
  c^.loop := lp;
  c^.sched := sched;
  c^.sock := InvalidSocket;
  c^.resp := NIL;
  c^.tlsCtx := NIL;
  c^.tlsSess := NIL;
  c^.stream := NIL;
  c^.streamId := 0;
  c^.recvLen := 0;
  c^.sendPtr := NIL;
  c^.sendTotal := 0;
  c^.sendPos := 0;
  c^.gotSettings := FALSE;
  c^.gotHeaders := FALSE;
  c^.gotEndStream := FALSE;
  c^.goawayRecv := FALSE;
  StrCopy(method, c^.method);
  c^.hasBody := withBody;
  c^.reqBody := bodyData;
  c^.reqBodyLen := bodyLen;
  c^.bodyLen := bodyLen;

  (* Store request parameters *)
  StrCopy(uri.host, c^.host);
  BuildAuthority(c, uri);
  ust := RequestPath(uri, c^.path, c^.pathLen);
  StrCopy(contentType, c^.contentType);
  StrCopy(authorization, c^.authorization);

  (* 3. Initialize H2 state — inline buffers and HPACK tables *)
  ByteBuf.Init(c^.outBuf, 1024);
  Http2Hpack.DynInit(c^.dynEnc, DefaultHeaderTableSize);
  Http2Hpack.DynInit(c^.dynDec, DefaultHeaderTableSize);
  c^.remoteMaxFrame := DefaultMaxFrameSize;
  ByteBuf.Init(c^.hdrBuf, 256);

  (* 4. Create promise *)
  pst := PromiseCreate(sched, c^.promise, outFuture);
  IF pst # Promise.OK THEN
    ByteBuf.Free(c^.outBuf);
    ByteBuf.Free(c^.hdrBuf);
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(H2ConnRec));
    RETURN HTTPClient.OutOfMemory
  END;

  (* 5. Create response *)
  ALLOCATE(c^.resp, TSIZE(Response));
  IF c^.resp = NIL THEN
    ByteBuf.Free(c^.outBuf);
    ByteBuf.Free(c^.hdrBuf);
    DEALLOCATE(ap, TSIZE(AddrRec));
    DEALLOCATE(c, TSIZE(H2ConnRec));
    RETURN HTTPClient.OutOfMemory
  END;
  c^.resp^.statusCode := 0;
  c^.resp^.headerCount := 0;
  c^.resp^.body := NIL;
  c^.resp^.contentLength := -1;

  (* 6. Create socket *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, c^.sock);
  IF sst # Sockets.OK THEN
    FailConn(c, 1);
    RETURN HTTPClient.ConnectFailed
  END;
  sst := SetNonBlocking(c^.sock, TRUE);

  (* 7. Set up TLS with ALPN "h2" *)
  tst := TLS.ContextCreate(c^.tlsCtx);
  IF tst # TLS.OK THEN
    FailConn(c, 6);
    RETURN HTTPClient.TLSFailed
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
    RETURN HTTPClient.TLSFailed
  END;
  tst := TLS.SetALPN(c^.tlsCtx, ADR(mALPN), 3);
  tst := TLS.SessionCreate(lp, sched, c^.tlsCtx, c^.sock,
                            c^.tlsSess);
  IF tst # TLS.OK THEN
    FailConn(c, 6);
    RETURN HTTPClient.TLSFailed
  END;
  tst := TLS.SetSNI(c^.tlsSess, uri.host);

  (* 8. Connect *)
  crc := m2_connect_ipv4(c^.sock,
                          ORD(ap^.addrV4[0]),
                          ORD(ap^.addrV4[1]),
                          ORD(ap^.addrV4[2]),
                          ORD(ap^.addrV4[3]),
                          uri.port);
  DEALLOCATE(ap, TSIZE(AddrRec));

  IF crc < 0 THEN
    FailConn(c, 1);
    RETURN HTTPClient.ConnectFailed
  END;

  est := EventLoop.WatchFd(lp, c^.sock, EvWrite, OnSocketEvent, c);
  IF est # EventLoop.OK THEN
    FailConn(c, 1);
    RETURN HTTPClient.ConnectFailed
  END;

  RETURN HTTPClient.OK
END DoConnect;

(* ── Public API ──────────────────────────────────────────── *)

PROCEDURE Get(lp: EventLoop.Loop; sched: Scheduler;
              VAR uri: URIRec;
              VAR outFuture: Future): Status;
VAR method: ARRAY [0..3] OF CHAR;
    dummy: ARRAY [0..0] OF CHAR;
BEGIN
  method[0] := 'G'; method[1] := 'E'; method[2] := 'T'; method[3] := 0C;
  dummy[0] := 0C;
  RETURN DoConnect(lp, sched, uri, method, FALSE, NIL, 0, dummy, dummy,
                   outFuture)
END Get;

PROCEDURE Put(lp: EventLoop.Loop; sched: Scheduler;
              VAR uri: URIRec;
              bodyData: ADDRESS; bodyLen: INTEGER;
              VAR contentType: ARRAY OF CHAR;
              VAR authorization: ARRAY OF CHAR;
              VAR outFuture: Future): Status;
VAR method: ARRAY [0..3] OF CHAR;
BEGIN
  method[0] := 'P'; method[1] := 'U'; method[2] := 'T'; method[3] := 0C;
  RETURN DoConnect(lp, sched, uri, method, TRUE, bodyData, bodyLen,
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
  RETURN DoConnect(lp, sched, uri, method, TRUE, bodyData, bodyLen,
                   contentType, authorization, outFuture)
END Post;

PROCEDURE Delete(lp: EventLoop.Loop; sched: Scheduler;
                 VAR uri: URIRec;
                 VAR authorization: ARRAY OF CHAR;
                 VAR outFuture: Future): Status;
VAR method: ARRAY [0..6] OF CHAR;
    dummy: ARRAY [0..0] OF CHAR;
BEGIN
  method[0] := 'D'; method[1] := 'E'; method[2] := 'L'; method[3] := 'E';
  method[4] := 'T'; method[5] := 'E'; method[6] := 0C;
  dummy[0] := 0C;
  RETURN DoConnect(lp, sched, uri, method, FALSE, NIL, 0, dummy,
                   authorization, outFuture)
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
  RETURN DoConnect(lp, sched, uri, method, TRUE, bodyData, bodyLen,
                   contentType, authorization, outFuture)
END Patch;

PROCEDURE FreeResponse(VAR resp: ResponsePtr);
BEGIN
  HTTPClient.FreeResponse(resp)
END FreeResponse;

PROCEDURE SetSkipVerify(skip: BOOLEAN);
BEGIN
  mSkipVerify := skip
END SetSkipVerify;

BEGIN
  mSkipVerify := FALSE;
  (* ALPN wire format: length-prefixed "h2" *)
  mALPN[0] := CHR(2);
  mALPN[1] := 'h';
  mALPN[2] := '2'
END H2Client.
