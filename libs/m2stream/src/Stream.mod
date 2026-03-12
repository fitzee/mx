IMPLEMENTATION MODULE Stream;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Poller IMPORT EvRead, EvWrite;
IMPORT EventLoop;
FROM Promise IMPORT Promise, Future, Value, Error,
                    PromiseCreate, Resolve, Reject;
IMPORT Promise;
IMPORT TLS;
FROM SocketsBridge IMPORT m2_send, m2_recv;
FROM Sockets IMPORT InvalidSocket, SHUT_WR;
IMPORT Sockets;

(* ── Internal types ──────────────────────────────────── *)

CONST
  OpNone     = 0;
  OpRead     = 1;
  OpWrite    = 2;
  OpWriteAll = 3;
  OpClose    = 4;

TYPE
  StreamPtr = POINTER TO StreamRec;

  StreamRec = RECORD
    kind     : StreamKind;
    state    : StreamState;
    fd       : INTEGER;
    lp       : ADDRESS;     (* EventLoop.Loop *)
    sched    : ADDRESS;     (* Scheduler *)
    watching : BOOLEAN;
    (* TLS *)
    tlsCtx   : ADDRESS;    (* TLS.TLSContext *)
    tlsSess  : ADDRESS;     (* TLS.TLSSession *)
    (* Pending async operation *)
    op       : INTEGER;
    promise  : ADDRESS;     (* Promise *)
    opBuf    : ADDRESS;
    opLen    : INTEGER;
    opSent   : INTEGER;
  END;

(* ── Pointer arithmetic helper ───────────────────────── *)

PROCEDURE OffsetPtr(base: ADDRESS; n: INTEGER): ADDRESS;
BEGIN
  RETURN VAL(ADDRESS, LONGCARD(base) + LONGCARD(VAL(CARDINAL, n)))
END OffsetPtr;

(* ── Sync (try-once) operations ──────────────────────── *)

PROCEDURE TryRead(s: Stream; buf: ADDRESS; max: INTEGER;
                  VAR got: INTEGER): Status;
VAR
  sp: StreamPtr;
  n: INTEGER;
  tst: TLS.Status;
  est: EventLoop.Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;
  IF (sp^.state = Closed) OR (sp^.state = Error) THEN
    RETURN StreamClosed
  END;

  IF sp^.kind = TLSStream THEN
    tst := TLS.Read(sp^.tlsSess, buf, max, n);
    IF tst = TLS.OK THEN
      got := n;
      RETURN OK
    ELSIF tst = TLS.Closed THEN
      RETURN StreamClosed
    ELSIF tst = TLS.WantRead THEN
      est := EventLoop.ModifyFd(sp^.lp, sp^.fd, EvRead);
      RETURN WouldBlock
    ELSIF tst = TLS.WantWrite THEN
      est := EventLoop.ModifyFd(sp^.lp, sp^.fd, EvWrite);
      RETURN WouldBlock
    ELSE
      sp^.state := Error;
      RETURN TLSError
    END
  ELSE
    n := m2_recv(sp^.fd, buf, max);
    IF n > 0 THEN
      got := n;
      RETURN OK
    ELSIF n = 0 THEN
      RETURN StreamClosed
    ELSE
      sp^.state := Error;
      RETURN SysError
    END
  END
END TryRead;

PROCEDURE TryWrite(s: Stream; buf: ADDRESS; len: INTEGER;
                   VAR sent: INTEGER): Status;
VAR
  sp: StreamPtr;
  n: INTEGER;
  tst: TLS.Status;
  est: EventLoop.Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;
  IF (sp^.state = Closed) OR (sp^.state = Error) THEN
    RETURN StreamClosed
  END;
  IF sp^.state = ShutdownWr THEN RETURN Invalid END;

  IF sp^.kind = TLSStream THEN
    tst := TLS.Write(sp^.tlsSess, buf, len, n);
    IF tst = TLS.OK THEN
      sent := n;
      RETURN OK
    ELSIF tst = TLS.WantRead THEN
      est := EventLoop.ModifyFd(sp^.lp, sp^.fd, EvRead);
      RETURN WouldBlock
    ELSIF tst = TLS.WantWrite THEN
      est := EventLoop.ModifyFd(sp^.lp, sp^.fd, EvWrite);
      RETURN WouldBlock
    ELSE
      sp^.state := Error;
      RETURN TLSError
    END
  ELSE
    n := m2_send(sp^.fd, buf, len);
    IF n > 0 THEN
      sent := n;
      RETURN OK
    ELSIF n = 0 THEN
      RETURN WouldBlock
    ELSE
      sp^.state := Error;
      RETURN SysError
    END
  END
END TryWrite;

(* ── Async watcher callback ──────────────────────────── *)

PROCEDURE ResolveOp(sp: StreamPtr; tag: INTEGER);
VAR v: Value; pst: Promise.Status;
BEGIN
  sp^.op := OpNone;
  v.tag := tag;
  v.ptr := NIL;
  pst := Resolve(sp^.promise, v)
END ResolveOp;

PROCEDURE RejectOp(sp: StreamPtr; code: INTEGER);
VAR e: Error; pst: Promise.Status;
BEGIN
  sp^.op := OpNone;
  e.code := code;
  e.ptr := NIL;
  pst := Reject(sp^.promise, e)
END RejectOp;

PROCEDURE OnStreamEvent(fd, events: INTEGER; user: ADDRESS);
VAR
  sp: StreamPtr;
  n: INTEGER;
  st: Status;
  tst: TLS.Status;
  est: EventLoop.Status;
  sst: Sockets.Status;
BEGIN
  sp := user;

  CASE sp^.op OF

    OpRead:
      st := TryRead(sp, sp^.opBuf, sp^.opLen, n);
      IF st = OK THEN
        ResolveOp(sp, n)
      ELSIF st = WouldBlock THEN
        (* watcher already adjusted by TryRead *)
      ELSIF st = StreamClosed THEN
        RejectOp(sp, 2)
      ELSE
        sp^.state := Error;
        RejectOp(sp, 1)
      END |

    OpWrite:
      st := TryWrite(sp, sp^.opBuf, sp^.opLen, n);
      IF st = OK THEN
        ResolveOp(sp, n)
      ELSIF st = WouldBlock THEN
        (* watcher already adjusted by TryWrite *)
      ELSE
        sp^.state := Error;
        RejectOp(sp, 1)
      END |

    OpWriteAll:
      st := TryWrite(sp,
                      OffsetPtr(sp^.opBuf, sp^.opSent),
                      sp^.opLen - sp^.opSent, n);
      IF st = OK THEN
        sp^.opSent := sp^.opSent + n;
        IF sp^.opSent >= sp^.opLen THEN
          ResolveOp(sp, sp^.opSent)
        ELSE
          est := EventLoop.ModifyFd(sp^.lp, sp^.fd, EvWrite)
        END
      ELSIF st = WouldBlock THEN
        (* watcher already adjusted by TryWrite *)
      ELSE
        sp^.state := Error;
        RejectOp(sp, 1)
      END |

    OpClose:
      IF sp^.kind = TLSStream THEN
        IF sp^.tlsSess # NIL THEN
          tst := TLS.Shutdown(sp^.tlsSess);
          IF tst = TLS.WantRead THEN
            est := EventLoop.ModifyFd(sp^.lp, sp^.fd, EvRead);
            RETURN
          ELSIF tst = TLS.WantWrite THEN
            est := EventLoop.ModifyFd(sp^.lp, sp^.fd, EvWrite);
            RETURN
          END
          (* OK or error — proceed to close *)
        END
      END;
      (* Unwatch, close socket, resolve *)
      IF sp^.watching THEN
        est := EventLoop.UnwatchFd(sp^.lp, sp^.fd);
        sp^.watching := FALSE
      END;
      sst := Sockets.CloseSocket(sp^.fd);
      sp^.fd := InvalidSocket;
      sp^.state := Closed;
      ResolveOp(sp, 0)

  ELSE
    (* OpNone — spurious event, ignore *)
  END
END OnStreamEvent;

(* ── Watcher registration helper ─────────────────────── *)

PROCEDURE EnsureWatcher(sp: StreamPtr; evts: INTEGER): Status;
VAR est: EventLoop.Status;
BEGIN
  IF NOT sp^.watching THEN
    est := EventLoop.WatchFd(sp^.lp, sp^.fd, evts,
                              OnStreamEvent, sp);
    IF est # EventLoop.OK THEN RETURN SysError END;
    sp^.watching := TRUE
  ELSE
    est := EventLoop.ModifyFd(sp^.lp, sp^.fd, evts)
  END;
  RETURN OK
END EnsureWatcher;

(* ── Creation ────────────────────────────────────────── *)

PROCEDURE CreateTCP(lp: ADDRESS; sched: Scheduler;
                    fd: INTEGER;
                    VAR out: Stream): Status;
VAR sp: StreamPtr;
BEGIN
  IF (lp = NIL) OR (sched = NIL) OR (fd < 0) THEN
    RETURN Invalid
  END;
  ALLOCATE(sp, TSIZE(StreamRec));
  IF sp = NIL THEN RETURN OutOfMemory END;
  sp^.kind := TCP;
  sp^.state := Open;
  sp^.fd := fd;
  sp^.lp := lp;
  sp^.sched := sched;
  sp^.watching := FALSE;
  sp^.tlsCtx := NIL;
  sp^.tlsSess := NIL;
  sp^.op := OpNone;
  sp^.promise := NIL;
  sp^.opBuf := NIL;
  sp^.opLen := 0;
  sp^.opSent := 0;
  out := sp;
  RETURN OK
END CreateTCP;

PROCEDURE CreateTLS(lp: ADDRESS; sched: Scheduler;
                    fd: INTEGER;
                    ctx: ADDRESS;
                    sess: ADDRESS;
                    VAR out: Stream): Status;
VAR sp: StreamPtr;
BEGIN
  IF (lp = NIL) OR (sched = NIL) OR (fd < 0) THEN
    RETURN Invalid
  END;
  IF (ctx = NIL) OR (sess = NIL) THEN RETURN Invalid END;
  ALLOCATE(sp, TSIZE(StreamRec));
  IF sp = NIL THEN RETURN OutOfMemory END;
  sp^.kind := TLSStream;
  sp^.state := Open;
  sp^.fd := fd;
  sp^.lp := lp;
  sp^.sched := sched;
  sp^.watching := FALSE;
  sp^.tlsCtx := ctx;
  sp^.tlsSess := sess;
  sp^.op := OpNone;
  sp^.promise := NIL;
  sp^.opBuf := NIL;
  sp^.opLen := 0;
  sp^.opSent := 0;
  out := sp;
  RETURN OK
END CreateTLS;

(* ── Async operations ────────────────────────────────── *)

PROCEDURE ReadAsync(s: Stream; buf: ADDRESS; max: INTEGER;
                    VAR out: Future): Status;
VAR
  sp: StreamPtr;
  pst: Promise.Status;
  wst: Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;
  IF sp^.op # OpNone THEN RETURN Invalid END;
  IF sp^.state # Open THEN RETURN Invalid END;

  pst := PromiseCreate(sp^.sched, sp^.promise, out);
  IF pst # Promise.OK THEN RETURN OutOfMemory END;

  sp^.op := OpRead;
  sp^.opBuf := buf;
  sp^.opLen := max;
  sp^.opSent := 0;

  wst := EnsureWatcher(sp, EvRead);
  IF wst # OK THEN
    sp^.op := OpNone;
    RETURN wst
  END;
  RETURN OK
END ReadAsync;

PROCEDURE WriteAsync(s: Stream; buf: ADDRESS; len: INTEGER;
                     VAR out: Future): Status;
VAR
  sp: StreamPtr;
  pst: Promise.Status;
  wst: Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;
  IF sp^.op # OpNone THEN RETURN Invalid END;
  IF (sp^.state # Open) THEN RETURN Invalid END;

  pst := PromiseCreate(sp^.sched, sp^.promise, out);
  IF pst # Promise.OK THEN RETURN OutOfMemory END;

  sp^.op := OpWrite;
  sp^.opBuf := buf;
  sp^.opLen := len;
  sp^.opSent := 0;

  wst := EnsureWatcher(sp, EvWrite);
  IF wst # OK THEN
    sp^.op := OpNone;
    RETURN wst
  END;
  RETURN OK
END WriteAsync;

PROCEDURE WriteAllAsync(s: Stream; buf: ADDRESS; len: INTEGER;
                        VAR out: Future): Status;
VAR
  sp: StreamPtr;
  pst: Promise.Status;
  wst: Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;
  IF sp^.op # OpNone THEN RETURN Invalid END;
  IF (sp^.state # Open) THEN RETURN Invalid END;

  pst := PromiseCreate(sp^.sched, sp^.promise, out);
  IF pst # Promise.OK THEN RETURN OutOfMemory END;

  sp^.op := OpWriteAll;
  sp^.opBuf := buf;
  sp^.opLen := len;
  sp^.opSent := 0;

  wst := EnsureWatcher(sp, EvWrite);
  IF wst # OK THEN
    sp^.op := OpNone;
    RETURN wst
  END;
  RETURN OK
END WriteAllAsync;

PROCEDURE CloseAsync(s: Stream;
                     VAR out: Future): Status;
VAR
  sp: StreamPtr;
  pst: Promise.Status;
  wst: Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;
  IF sp^.op # OpNone THEN RETURN Invalid END;

  pst := PromiseCreate(sp^.sched, sp^.promise, out);
  IF pst # Promise.OK THEN RETURN OutOfMemory END;

  sp^.op := OpClose;
  sp^.opBuf := NIL;
  sp^.opLen := 0;
  sp^.opSent := 0;

  IF sp^.kind = TLSStream THEN
    (* Start async TLS shutdown; need writable first *)
    wst := EnsureWatcher(sp, EvWrite);
    IF wst # OK THEN
      sp^.op := OpNone;
      RETURN wst
    END
  ELSE
    (* TCP: close immediately via microtask *)
    wst := EnsureWatcher(sp, EvWrite);
    IF wst # OK THEN
      sp^.op := OpNone;
      RETURN wst
    END
  END;
  RETURN OK
END CloseAsync;

(* ── Sync helpers ────────────────────────────────────── *)

PROCEDURE ShutdownWrite(s: Stream): Status;
VAR
  sp: StreamPtr;
  tst: TLS.Status;
  sst: Sockets.Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;
  IF sp^.state # Open THEN RETURN Invalid END;

  IF sp^.kind = TLSStream THEN
    IF sp^.tlsSess # NIL THEN
      tst := TLS.Shutdown(sp^.tlsSess)
      (* Ignore WantRead/WantWrite — best-effort *)
    END
  END;
  sst := Sockets.Shutdown(sp^.fd, SHUT_WR);
  sp^.state := ShutdownWr;
  RETURN OK
END ShutdownWrite;

PROCEDURE GetState(s: Stream): StreamState;
VAR sp: StreamPtr;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Error END;
  RETURN sp^.state
END GetState;

PROCEDURE GetFd(s: Stream): INTEGER;
VAR sp: StreamPtr;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN InvalidSocket END;
  RETURN sp^.fd
END GetFd;

PROCEDURE GetKind(s: Stream): StreamKind;
VAR sp: StreamPtr;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN TCP END;
  RETURN sp^.kind
END GetKind;

PROCEDURE Destroy(VAR s: Stream): Status;
VAR
  sp: StreamPtr;
  est: EventLoop.Status;
  sst: Sockets.Status;
  tst: TLS.Status;
BEGIN
  sp := s;
  IF sp = NIL THEN RETURN Invalid END;

  (* TLS cleanup *)
  IF sp^.kind = TLSStream THEN
    IF sp^.tlsSess # NIL THEN
      tst := TLS.Shutdown(sp^.tlsSess);
      tst := TLS.SessionDestroy(sp^.tlsSess);
      sp^.tlsSess := NIL
    END;
    IF sp^.tlsCtx # NIL THEN
      tst := TLS.ContextDestroy(sp^.tlsCtx);
      sp^.tlsCtx := NIL
    END
  END;

  (* If Stream owns the watcher, unwatch and close socket *)
  IF sp^.watching THEN
    IF sp^.fd # InvalidSocket THEN
      est := EventLoop.UnwatchFd(sp^.lp, sp^.fd);
      sst := Sockets.CloseSocket(sp^.fd);
      sp^.fd := InvalidSocket
    END;
    sp^.watching := FALSE
  END;

  sp^.state := Closed;
  DEALLOCATE(sp, TSIZE(StreamRec));
  s := NIL;
  RETURN OK
END Destroy;

BEGIN
  (* no module initialization needed *)
END Stream.
