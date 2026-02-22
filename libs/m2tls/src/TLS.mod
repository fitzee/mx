IMPLEMENTATION MODULE TLS;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM TlsBridge IMPORT m2_tls_init,
    m2_tls_ctx_create, m2_tls_ctx_destroy,
    m2_tls_ctx_set_verify, m2_tls_ctx_set_min_version,
    m2_tls_ctx_load_system_roots, m2_tls_ctx_load_ca_file,
    m2_tls_ctx_set_client_cert,
    m2_tls_ctx_create_server, m2_tls_ctx_set_server_cert,
    m2_tls_ctx_set_alpn, m2_tls_ctx_set_alpn_server,
    m2_tls_get_alpn,
    m2_tls_session_create, m2_tls_session_create_server,
    m2_tls_session_destroy,
    m2_tls_session_set_sni,
    m2_tls_handshake, m2_tls_read, m2_tls_write, m2_tls_shutdown,
    m2_tls_get_verify_result, m2_tls_get_peer_summary,
    m2_tls_get_last_error;
IMPORT EventLoop;
FROM Poller IMPORT EvRead, EvWrite;
IMPORT Scheduler;
FROM Promise IMPORT Future, Promise, Value, Error,
                    PromiseCreate, Resolve, Reject;
IMPORT Promise;

(* ── Internal types ──────────────────────────────────────────────── *)

CONST
  OpNone      = 0;
  OpHandshake = 1;
  OpRead      = 2;
  OpWrite     = 3;
  OpWriteAll  = 4;

  ErrSys      = 1;
  ErrVerify   = 2;
  ErrClosed   = 3;

TYPE
  SessRec = RECORD
    ssl:      ADDRESS;               (* SSL* from C bridge *)
    lp:       EventLoop.Loop;
    sched:    Scheduler.Scheduler;
    fd:       INTEGER;
    op:       INTEGER;               (* OpNone..OpWriteAll *)
    promise:  Promise;
    rdBuf:    ADDRESS;
    rdMax:    INTEGER;
    wrBuf:    ADDRESS;
    wrLen:    INTEGER;
    wrSent:   INTEGER;
    watching: BOOLEAN;
  END;

  SessPtr = POINTER TO SessRec;

(* ── Helper: unwatch fd ──────────────────────────────────────────── *)

PROCEDURE UnwatchSess(sp: SessPtr);
VAR est: EventLoop.Status;
BEGIN
  IF sp^.watching THEN
    est := EventLoop.UnwatchFd(sp^.lp, sp^.fd);
    sp^.watching := FALSE
  END
END UnwatchSess;

(* ── Helper: resolve pending promise ─────────────────────────────── *)

PROCEDURE ResolveSess(sp: SessPtr; tag: INTEGER);
VAR v: Value; dummy: Promise.Status;
BEGIN
  UnwatchSess(sp);
  v.tag := tag;
  v.ptr := NIL;
  dummy := Resolve(sp^.promise, v);
  sp^.op := OpNone
END ResolveSess;

(* ── Helper: reject pending promise ──────────────────────────────── *)

PROCEDURE RejectSess(sp: SessPtr; code: INTEGER);
VAR e: Error; dummy: Promise.Status;
BEGIN
  UnwatchSess(sp);
  e.code := code;
  e.ptr := NIL;
  dummy := Reject(sp^.promise, e);
  sp^.op := OpNone
END RejectSess;

(* ── Helper: set watcher direction ───────────────────────────────── *)

PROCEDURE WatchDir(sp: SessPtr; evMask: INTEGER);
VAR est: EventLoop.Status;
BEGIN
  IF sp^.watching THEN
    est := EventLoop.ModifyFd(sp^.lp, sp^.fd, evMask)
  ELSE
    est := EventLoop.WatchFd(sp^.lp, sp^.fd, evMask,
                              OnTLSEvent, sp);
    IF est = EventLoop.OK THEN
      sp^.watching := TRUE
    END
  END
END WatchDir;

(* ── Retry: handshake ────────────────────────────────────────────── *)

PROCEDURE RetryHandshake(sp: SessPtr);
VAR rc: INTEGER;
BEGIN
  rc := m2_tls_handshake(sp^.ssl);
  IF rc = 0 THEN
    ResolveSess(sp, 0)
  ELSIF rc = 1 THEN
    WatchDir(sp, EvRead)
  ELSIF rc = 2 THEN
    WatchDir(sp, EvWrite)
  ELSIF rc = -2 THEN
    RejectSess(sp, ErrVerify)
  ELSE
    RejectSess(sp, ErrSys)
  END
END RetryHandshake;

(* ── Retry: read ─────────────────────────────────────────────────── *)

PROCEDURE RetryRead(sp: SessPtr);
VAR n: INTEGER;
BEGIN
  n := m2_tls_read(sp^.ssl, sp^.rdBuf, sp^.rdMax);
  IF n > 0 THEN
    ResolveSess(sp, n)
  ELSIF n = 0 THEN
    RejectSess(sp, ErrClosed)
  ELSIF n = -1 THEN
    WatchDir(sp, EvRead)
  ELSIF n = -2 THEN
    WatchDir(sp, EvWrite)
  ELSE
    RejectSess(sp, ErrSys)
  END
END RetryRead;

(* ── Retry: write ────────────────────────────────────────────────── *)

PROCEDURE RetryWrite(sp: SessPtr);
VAR n: INTEGER;
BEGIN
  n := m2_tls_write(sp^.ssl, sp^.wrBuf, sp^.wrLen);
  IF n > 0 THEN
    ResolveSess(sp, n)
  ELSIF n = -1 THEN
    WatchDir(sp, EvRead)
  ELSIF n = -2 THEN
    WatchDir(sp, EvWrite)
  ELSE
    RejectSess(sp, ErrSys)
  END
END RetryWrite;

(* ── Retry: write-all ────────────────────────────────────────────── *)

PROCEDURE RetryWriteAll(sp: SessPtr);
VAR n: INTEGER; base: ADDRESS;
BEGIN
  (* Compute pointer into buffer at current offset *)
  base := sp^.wrBuf + sp^.wrSent;
  n := m2_tls_write(sp^.ssl, base, sp^.wrLen - sp^.wrSent);
  IF n > 0 THEN
    sp^.wrSent := sp^.wrSent + n;
    IF sp^.wrSent >= sp^.wrLen THEN
      ResolveSess(sp, sp^.wrSent)
    ELSE
      (* More to send; keep watching *)
      WatchDir(sp, EvWrite)
    END
  ELSIF n = -1 THEN
    WatchDir(sp, EvRead)
  ELSIF n = -2 THEN
    WatchDir(sp, EvWrite)
  ELSE
    RejectSess(sp, ErrSys)
  END
END RetryWriteAll;

(* ── Watcher callback ────────────────────────────────────────────── *)

PROCEDURE OnTLSEvent(fd, events: INTEGER; user: ADDRESS);
VAR sp: SessPtr;
BEGIN
  sp := user;
  CASE sp^.op OF
    OpHandshake: RetryHandshake(sp) |
    OpRead:      RetryRead(sp) |
    OpWrite:     RetryWrite(sp) |
    OpWriteAll:  RetryWriteAll(sp)
  ELSE
    (* OpNone — spurious; unwatch *)
    UnwatchSess(sp)
  END
END OnTLSEvent;

(* ── Context lifecycle ───────────────────────────────────────────── *)

PROCEDURE ContextCreate(VAR out: TLSContext): Status;
BEGIN
  out := m2_tls_ctx_create();
  IF out = NIL THEN RETURN OutOfMemory END;
  RETURN OK
END ContextCreate;

PROCEDURE ContextDestroy(VAR ctx: TLSContext): Status;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  m2_tls_ctx_destroy(ctx);
  ctx := NIL;
  RETURN OK
END ContextDestroy;

(* ── Context configuration ───────────────────────────────────────── *)

PROCEDURE SetVerifyMode(ctx: TLSContext; mode: VerifyMode): Status;
VAR rc: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  IF mode = VerifyPeer THEN
    rc := m2_tls_ctx_set_verify(ctx, 1)
  ELSE
    rc := m2_tls_ctx_set_verify(ctx, 0)
  END;
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END SetVerifyMode;

PROCEDURE SetMinVersion(ctx: TLSContext; v: TLSVersion): Status;
VAR rc, ver: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  CASE v OF
    TLS10: ver := 0 |
    TLS11: ver := 1 |
    TLS12: ver := 2 |
    TLS13: ver := 3
  ELSE
    RETURN Invalid
  END;
  rc := m2_tls_ctx_set_min_version(ctx, ver);
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END SetMinVersion;

PROCEDURE LoadSystemRoots(ctx: TLSContext): Status;
VAR rc: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  rc := m2_tls_ctx_load_system_roots(ctx);
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END LoadSystemRoots;

PROCEDURE LoadCAFile(ctx: TLSContext;
                     VAR path: ARRAY OF CHAR): Status;
VAR rc: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  rc := m2_tls_ctx_load_ca_file(ctx, ADR(path));
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END LoadCAFile;

PROCEDURE SetClientCert(ctx: TLSContext;
                        VAR certPath, keyPath: ARRAY OF CHAR): Status;
VAR rc: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  rc := m2_tls_ctx_set_client_cert(ctx, ADR(certPath), ADR(keyPath));
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END SetClientCert;

(* ── Server context ──────────────────────────────────────────────── *)

PROCEDURE ContextCreateServer(VAR out: TLSContext): Status;
BEGIN
  out := m2_tls_ctx_create_server();
  IF out = NIL THEN RETURN OutOfMemory END;
  RETURN OK
END ContextCreateServer;

PROCEDURE SetServerCert(ctx: TLSContext;
                        VAR certPath, keyPath: ARRAY OF CHAR): Status;
VAR rc: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  rc := m2_tls_ctx_set_server_cert(ctx, ADR(certPath), ADR(keyPath));
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END SetServerCert;

(* ── ALPN ────────────────────────────────────────────────────────── *)

PROCEDURE SetALPN(ctx: TLSContext;
                  protos: ADDRESS; protosLen: INTEGER): Status;
VAR rc: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  rc := m2_tls_ctx_set_alpn(ctx, protos, protosLen);
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END SetALPN;

PROCEDURE SetALPNServer(ctx: TLSContext;
                        protos: ADDRESS; protosLen: INTEGER): Status;
VAR rc: INTEGER;
BEGIN
  IF ctx = NIL THEN RETURN Invalid END;
  rc := m2_tls_ctx_set_alpn_server(ctx, protos, protosLen);
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END SetALPNServer;

(* ── Session lifecycle ───────────────────────────────────────────── *)

PROCEDURE SessionCreate(lp: Loop; sched: Scheduler.Scheduler;
                        ctx: TLSContext; fd: INTEGER;
                        VAR out: TLSSession): Status;
VAR sp: SessPtr; ssl: ADDRESS;
BEGIN
  IF (ctx = NIL) OR (lp = NIL) OR (sched = NIL) THEN
    out := NIL;
    RETURN Invalid
  END;
  ssl := m2_tls_session_create(ctx, fd);
  IF ssl = NIL THEN
    out := NIL;
    RETURN SysError
  END;
  ALLOCATE(sp, TSIZE(SessRec));
  IF sp = NIL THEN
    m2_tls_session_destroy(ssl);
    out := NIL;
    RETURN OutOfMemory
  END;
  sp^.ssl := ssl;
  sp^.lp := lp;
  sp^.sched := sched;
  sp^.fd := fd;
  sp^.op := OpNone;
  sp^.rdBuf := NIL;
  sp^.rdMax := 0;
  sp^.wrBuf := NIL;
  sp^.wrLen := 0;
  sp^.wrSent := 0;
  sp^.watching := FALSE;
  out := sp;
  RETURN OK
END SessionCreate;

PROCEDURE SessionCreateServer(lp: Loop; sched: Scheduler.Scheduler;
                              ctx: TLSContext; fd: INTEGER;
                              VAR out: TLSSession): Status;
VAR sp: SessPtr; ssl: ADDRESS;
BEGIN
  IF (ctx = NIL) OR (lp = NIL) OR (sched = NIL) THEN
    out := NIL;
    RETURN Invalid
  END;
  ssl := m2_tls_session_create_server(ctx, fd);
  IF ssl = NIL THEN
    out := NIL;
    RETURN SysError
  END;
  ALLOCATE(sp, TSIZE(SessRec));
  IF sp = NIL THEN
    m2_tls_session_destroy(ssl);
    out := NIL;
    RETURN OutOfMemory
  END;
  sp^.ssl := ssl;
  sp^.lp := lp;
  sp^.sched := sched;
  sp^.fd := fd;
  sp^.op := OpNone;
  sp^.rdBuf := NIL;
  sp^.rdMax := 0;
  sp^.wrBuf := NIL;
  sp^.wrLen := 0;
  sp^.wrSent := 0;
  sp^.watching := FALSE;
  out := sp;
  RETURN OK
END SessionCreateServer;

PROCEDURE SessionDestroy(VAR s: TLSSession): Status;
VAR sp: SessPtr;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  (* Cancel any pending async operation *)
  IF sp^.op # OpNone THEN
    RejectSess(sp, ErrSys)
  END;
  UnwatchSess(sp);
  m2_tls_session_destroy(sp^.ssl);
  sp^.ssl := NIL;
  DEALLOCATE(sp, TSIZE(SessRec));
  s := NIL;
  RETURN OK
END SessionDestroy;

PROCEDURE SetSNI(s: TLSSession; VAR host: ARRAY OF CHAR): Status;
VAR sp: SessPtr; rc: INTEGER;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  rc := m2_tls_session_set_sni(sp^.ssl, ADR(host));
  IF rc < 0 THEN RETURN SysError END;
  RETURN OK
END SetSNI;

(* ── Sync operations ─────────────────────────────────────────────── *)

PROCEDURE Handshake(s: TLSSession): Status;
VAR sp: SessPtr; rc: INTEGER;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  rc := m2_tls_handshake(sp^.ssl);
  IF rc = 0 THEN RETURN OK
  ELSIF rc = 1 THEN RETURN WantRead
  ELSIF rc = 2 THEN RETURN WantWrite
  ELSIF rc = -2 THEN RETURN VerifyFailed
  ELSE RETURN SysError
  END
END Handshake;

PROCEDURE Read(s: TLSSession; buf: ADDRESS; max: INTEGER;
               VAR got: INTEGER): Status;
VAR sp: SessPtr; n: INTEGER;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  got := 0;
  n := m2_tls_read(sp^.ssl, buf, max);
  IF n > 0 THEN
    got := n;
    RETURN OK
  ELSIF n = 0 THEN
    RETURN Closed
  ELSIF n = -1 THEN
    RETURN WantRead
  ELSIF n = -2 THEN
    RETURN WantWrite
  ELSE
    RETURN SysError
  END
END Read;

PROCEDURE Write(s: TLSSession; buf: ADDRESS; len: INTEGER;
                VAR sent: INTEGER): Status;
VAR sp: SessPtr; n: INTEGER;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  sent := 0;
  n := m2_tls_write(sp^.ssl, buf, len);
  IF n > 0 THEN
    sent := n;
    RETURN OK
  ELSIF n = -1 THEN
    RETURN WantRead
  ELSIF n = -2 THEN
    RETURN WantWrite
  ELSE
    RETURN SysError
  END
END Write;

PROCEDURE Shutdown(s: TLSSession): Status;
VAR sp: SessPtr; rc: INTEGER;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  rc := m2_tls_shutdown(sp^.ssl);
  IF rc = 0 THEN RETURN OK
  ELSIF rc = 1 THEN RETURN WantRead
  ELSIF rc = 2 THEN RETURN WantWrite
  ELSE RETURN SysError
  END
END Shutdown;

(* ── Async operations ────────────────────────────────────────────── *)

PROCEDURE StartAsync(sp: SessPtr; opKind: INTEGER;
                     VAR out: Future): Status;
VAR pst: Promise.Status; f: Future; p: Promise;
BEGIN
  IF sp^.op # OpNone THEN
    out := NIL;
    RETURN Invalid   (* another operation is already pending *)
  END;
  pst := PromiseCreate(sp^.sched, p, f);
  IF pst # Promise.OK THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  sp^.op := opKind;
  sp^.promise := p;
  out := f;
  RETURN OK
END StartAsync;

PROCEDURE HandshakeAsync(s: TLSSession; VAR out: Future): Status;
VAR sp: SessPtr; rc: INTEGER; st: Status;
    v: Value; e: Error; dummy: Promise.Status;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;

  (* Try sync first *)
  rc := m2_tls_handshake(sp^.ssl);

  IF rc = 0 THEN
    (* Already complete — return settled future *)
    st := StartAsync(sp, OpHandshake, out);
    IF st # OK THEN RETURN st END;
    v.tag := 0; v.ptr := NIL;
    dummy := Resolve(sp^.promise, v);
    sp^.op := OpNone;
    RETURN OK
  END;

  IF (rc # 1) AND (rc # 2) THEN
    (* Error — return rejected future *)
    st := StartAsync(sp, OpHandshake, out);
    IF st # OK THEN RETURN st END;
    e.ptr := NIL;
    IF rc = -2 THEN e.code := ErrVerify ELSE e.code := ErrSys END;
    dummy := Reject(sp^.promise, e);
    sp^.op := OpNone;
    IF rc = -2 THEN RETURN VerifyFailed ELSE RETURN SysError END
  END;

  (* WANT_READ or WANT_WRITE — register watcher *)
  st := StartAsync(sp, OpHandshake, out);
  IF st # OK THEN RETURN st END;
  IF rc = 1 THEN
    WatchDir(sp, EvRead)
  ELSE
    WatchDir(sp, EvWrite)
  END;
  RETURN OK
END HandshakeAsync;

PROCEDURE ReadAsync(s: TLSSession; buf: ADDRESS; max: INTEGER;
                    VAR out: Future): Status;
VAR sp: SessPtr; n: INTEGER; st: Status;
    v: Value; e: Error; dummy: Promise.Status;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  sp^.rdBuf := buf;
  sp^.rdMax := max;

  (* Try sync first *)
  n := m2_tls_read(sp^.ssl, buf, max);

  IF n > 0 THEN
    st := StartAsync(sp, OpRead, out);
    IF st # OK THEN RETURN st END;
    v.tag := n; v.ptr := NIL;
    dummy := Resolve(sp^.promise, v);
    sp^.op := OpNone;
    RETURN OK
  END;

  IF n = 0 THEN
    st := StartAsync(sp, OpRead, out);
    IF st # OK THEN RETURN st END;
    e.code := ErrClosed; e.ptr := NIL;
    dummy := Reject(sp^.promise, e);
    sp^.op := OpNone;
    RETURN Closed
  END;

  IF (n # -1) AND (n # -2) THEN
    st := StartAsync(sp, OpRead, out);
    IF st # OK THEN RETURN st END;
    e.code := ErrSys; e.ptr := NIL;
    dummy := Reject(sp^.promise, e);
    sp^.op := OpNone;
    RETURN SysError
  END;

  (* WANT_READ or WANT_WRITE *)
  st := StartAsync(sp, OpRead, out);
  IF st # OK THEN RETURN st END;
  IF n = -1 THEN
    WatchDir(sp, EvRead)
  ELSE
    WatchDir(sp, EvWrite)
  END;
  RETURN OK
END ReadAsync;

PROCEDURE WriteAsync(s: TLSSession; buf: ADDRESS; len: INTEGER;
                     VAR out: Future): Status;
VAR sp: SessPtr; n: INTEGER; st: Status;
    v: Value; e: Error; dummy: Promise.Status;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  sp^.wrBuf := buf;
  sp^.wrLen := len;

  (* Try sync first *)
  n := m2_tls_write(sp^.ssl, buf, len);

  IF n > 0 THEN
    st := StartAsync(sp, OpWrite, out);
    IF st # OK THEN RETURN st END;
    v.tag := n; v.ptr := NIL;
    dummy := Resolve(sp^.promise, v);
    sp^.op := OpNone;
    RETURN OK
  END;

  IF (n # -1) AND (n # -2) THEN
    st := StartAsync(sp, OpWrite, out);
    IF st # OK THEN RETURN st END;
    e.code := ErrSys; e.ptr := NIL;
    dummy := Reject(sp^.promise, e);
    sp^.op := OpNone;
    RETURN SysError
  END;

  st := StartAsync(sp, OpWrite, out);
  IF st # OK THEN RETURN st END;
  IF n = -1 THEN
    WatchDir(sp, EvRead)
  ELSE
    WatchDir(sp, EvWrite)
  END;
  RETURN OK
END WriteAsync;

PROCEDURE WriteAllAsync(s: TLSSession; buf: ADDRESS; len: INTEGER;
                        VAR out: Future): Status;
VAR sp: SessPtr; n: INTEGER; st: Status;
    v: Value; e: Error; dummy: Promise.Status;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  sp^.wrBuf := buf;
  sp^.wrLen := len;
  sp^.wrSent := 0;

  (* Try sync first *)
  n := m2_tls_write(sp^.ssl, buf, len);

  IF n > 0 THEN
    sp^.wrSent := n;
    IF sp^.wrSent >= len THEN
      st := StartAsync(sp, OpWriteAll, out);
      IF st # OK THEN RETURN st END;
      v.tag := sp^.wrSent; v.ptr := NIL;
      dummy := Resolve(sp^.promise, v);
      sp^.op := OpNone;
      RETURN OK
    END;
    (* Partial write — need async completion *)
    st := StartAsync(sp, OpWriteAll, out);
    IF st # OK THEN RETURN st END;
    WatchDir(sp, EvWrite);
    RETURN OK
  END;

  IF (n # -1) AND (n # -2) THEN
    st := StartAsync(sp, OpWriteAll, out);
    IF st # OK THEN RETURN st END;
    e.code := ErrSys; e.ptr := NIL;
    dummy := Reject(sp^.promise, e);
    sp^.op := OpNone;
    RETURN SysError
  END;

  st := StartAsync(sp, OpWriteAll, out);
  IF st # OK THEN RETURN st END;
  IF n = -1 THEN
    WatchDir(sp, EvRead)
  ELSE
    WatchDir(sp, EvWrite)
  END;
  RETURN OK
END WriteAllAsync;

(* ── Diagnostics ─────────────────────────────────────────────────── *)

PROCEDURE GetPeerSummary(s: TLSSession;
                         VAR out: ARRAY OF CHAR): Status;
VAR sp: SessPtr; rc: INTEGER;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  rc := m2_tls_get_peer_summary(sp^.ssl, ADR(out), HIGH(out) + 1);
  IF rc < 0 THEN RETURN Invalid END;
  RETURN OK
END GetPeerSummary;

PROCEDURE GetALPN(s: TLSSession;
                  VAR out: ARRAY OF CHAR;
                  VAR got: INTEGER): Status;
VAR sp: SessPtr;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  got := m2_tls_get_alpn(sp^.ssl, ADR(out), HIGH(out) + 1);
  RETURN OK
END GetALPN;

PROCEDURE GetVerifyResult(s: TLSSession): INTEGER;
VAR sp: SessPtr;
BEGIN
  IF s = NIL THEN RETURN -1 END;
  sp := s;
  RETURN m2_tls_get_verify_result(sp^.ssl)
END GetVerifyResult;

PROCEDURE GetLastError(VAR out: ARRAY OF CHAR);
BEGIN
  m2_tls_get_last_error(ADR(out), HIGH(out) + 1)
END GetLastError;

END TLS.
