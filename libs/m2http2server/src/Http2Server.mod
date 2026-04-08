IMPLEMENTATION MODULE Http2Server;

  FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
  FROM Storage IMPORT ALLOCATE, DEALLOCATE;
  FROM ByteBuf IMPORT Buf, Init, Free, Clear, Len, AsView;
  FROM Http2Types IMPORT Settings, InitDefaultSettings,
                          DefaultWindowSize;
  FROM Http2ServerTypes IMPORT ServerOpts, Status, HandlerProc,
                                MiddlewareProc, MaxConns,
                                MaxStreamSlots, Request, Response,
                                InitDefaultOpts, InitResponse,
                                FreeResponse;
  FROM Http2Router IMPORT Router, RouterInit, AddRoute AS RouterAddRoute,
                           Dispatch;
  FROM Http2Middleware IMPORT Chain, ChainInit,
                              ChainAdd AS MwChainAdd, ChainRun;
  FROM Http2ServerConn IMPORT ConnPtr, ConnRec, ConnCreate, ConnClose,
                               ConnDrain, ConnFlush, ConnOnEvent,
                               ConnCreateTest, ConnFeedBytes,
                               CpHandshaking, CpPreface, CpSettings,
                               CpOpen, CpGoaway, CpClosed,
                               SetServerDispatch, SetConnCleanup;
  FROM Http2ServerMetrics IMPORT Metrics, MetricsInit,
                                  IncConnsAccepted, IncConnsActive,
                                  DecConnsActive, IncConnsClosed,
                                  IncConnsRejected;
  FROM Http2ServerLog IMPORT LogInit, LogConn, LogProtocol;
  FROM Log IMPORT Logger;
  FROM Sockets IMPORT Socket, SockAddr, SocketCreate, Bind, Listen,
                       Accept, CloseSocket, SetNonBlocking,
                       AF_INET, SOCK_STREAM, InvalidSocket;
  FROM SocketsBridge IMPORT m2_set_reuseport;
  FROM EventLoop IMPORT Loop, WatchFd, UnwatchFd, Run, Stop,
                         Create AS LoopCreate,
                         Destroy AS LoopDestroy,
                         GetScheduler, SetTimeout, CancelTimer;
  FROM Timers IMPORT TimerId;
  FROM Poller IMPORT EvRead, EvWrite;
  FROM TLS IMPORT TLSContext, TLSSession,
                   ContextCreateServer, SetServerCert,
                   SetALPNServer, ContextDestroy,
                   SessionCreateServer, Handshake, SessionDestroy,
                   MaxALPNLen;
  IMPORT Scheduler;

  CONST
    MaxServerConns = MaxConns;

  TYPE
    ServerRec = RECORD
      opts:         ServerOpts;
      router:       Router;
      middleware:    Chain;
      metrics:      Metrics;
      lg:           Logger;
      loop:         Loop;
      tlsCtx:       TLSContext;
      listenSock:   Socket;
      conns:        ARRAY [0..MaxServerConns-1] OF ConnPtr;
      numConns:     CARDINAL;
      nextConnId:   CARDINAL;
      connLimit:    CARDINAL;    (* effective max connections, <= MaxServerConns *)
      streamLimit:  CARDINAL;    (* effective max streams per conn, <= MaxStreamSlots *)
      running:      BOOLEAN;
      draining:     BOOLEAN;
    END;

    ServerRecPtr = POINTER TO ServerRec;

  (* ── Dispatch bridge for Http2ServerConn ──────────── *)

  (* This is the procedure that ConnRec calls back into to
     dispatch a request through router + middleware. *)
  PROCEDURE DoDispatch(serverAddr: ADDRESS;
                       VAR req: Request;
                       VAR resp: Response);
  VAR
    sp: ServerRecPtr;
  BEGIN
    sp := ServerRecPtr(serverAddr);
    IF sp = NIL THEN
      resp.status := 500;
      RETURN;
    END;
    (* Find matching route and run through middleware *)
    ChainRun(sp^.middleware, req, resp,
             DispatchToRouter, ADDRESS(sp));
  END DoDispatch;

  (* Handler that ChainRun calls after all middleware passes *)
  PROCEDURE DispatchToRouter(VAR req: Request;
                             VAR resp: Response;
                             ctx: ADDRESS);
  VAR
    sp: ServerRecPtr;
  BEGIN
    sp := ServerRecPtr(ctx);
    Dispatch(sp^.router, req, resp);
  END DispatchToRouter;

  (* ── ALPN wire format for "h2" ───────────────────────── *)

  VAR
    alpnH2: ARRAY [0..2] OF CHAR;  (* wire: 02 68 32 *)

  (* ── Create ──────────────────────────────────────────── *)

  PROCEDURE Create(VAR opts: ServerOpts;
                   VAR out: Server): Status;
  VAR
    sp: ServerRecPtr;
    tlsSt: TLS.Status;
    sockSt: Sockets.Status;
    loopSt: EventLoop.Status;
    i: CARDINAL;
    rc: INTEGER;
  BEGIN
    ALLOCATE(sp, TSIZE(ServerRec));
    IF sp = NIL THEN
      RETURN OutOfMemory;
    END;

    sp^.opts := opts;
    RouterInit(sp^.router);
    ChainInit(sp^.middleware);
    MetricsInit(sp^.metrics);
    LogInit(sp^.lg);
    sp^.listenSock := InvalidSocket;
    sp^.numConns := 0;
    sp^.nextConnId := 1;
    sp^.running := FALSE;
    sp^.draining := FALSE;

    (* Compute effective limits: clamp to compile-time upper bounds *)
    sp^.connLimit := opts.maxConns;
    IF (sp^.connLimit = 0) OR (sp^.connLimit > MaxServerConns) THEN
      sp^.connLimit := MaxServerConns;
    END;
    sp^.streamLimit := opts.maxStreams;
    IF (sp^.streamLimit = 0) OR (sp^.streamLimit > MaxStreamSlots) THEN
      sp^.streamLimit := MaxStreamSlots;
    END;

    FOR i := 0 TO MaxServerConns - 1 DO
      sp^.conns[i] := NIL;
    END;

    (* Create event loop *)
    loopSt := LoopCreate(sp^.loop);
    IF loopSt # EventLoop.OK THEN
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN SysError;
    END;

    (* Create TLS server context *)
    tlsSt := ContextCreateServer(sp^.tlsCtx);
    IF tlsSt # TLS.OK THEN
      LoopDestroy(sp^.loop);
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN TLSFailed;
    END;

    (* Load cert + key *)
    tlsSt := SetServerCert(sp^.tlsCtx, opts.certPath, opts.keyPath);
    IF tlsSt # TLS.OK THEN
      ContextDestroy(sp^.tlsCtx);
      LoopDestroy(sp^.loop);
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN TLSFailed;
    END;

    (* Set ALPN to advertise h2 *)
    tlsSt := SetALPNServer(sp^.tlsCtx, ADR(alpnH2), 3);
    IF tlsSt # TLS.OK THEN
      ContextDestroy(sp^.tlsCtx);
      LoopDestroy(sp^.loop);
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN ALPNFailed;
    END;

    (* Create and bind listen socket *)
    sockSt := SocketCreate(AF_INET, SOCK_STREAM, sp^.listenSock);
    IF sockSt # Sockets.OK THEN
      ContextDestroy(sp^.tlsCtx);
      LoopDestroy(sp^.loop);
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN SysError;
    END;

    (* Enable SO_REUSEPORT for multi-listener support *)
    rc := m2_set_reuseport(INTEGER(sp^.listenSock), 1);

    sockSt := Bind(sp^.listenSock, opts.port);
    IF sockSt # Sockets.OK THEN
      CloseSocket(sp^.listenSock);
      ContextDestroy(sp^.tlsCtx);
      LoopDestroy(sp^.loop);
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN SysError;
    END;

    sockSt := Listen(sp^.listenSock, 1024);
    IF sockSt # Sockets.OK THEN
      CloseSocket(sp^.listenSock);
      ContextDestroy(sp^.tlsCtx);
      LoopDestroy(sp^.loop);
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN SysError;
    END;

    sockSt := SetNonBlocking(sp^.listenSock, TRUE);

    SetServerDispatch(DoDispatch);
    SetConnCleanup(DoCleanup);

    out := Server(sp);
    RETURN OK;
  END Create;

  (* ── AddRoute ────────────────────────────────────────── *)

  PROCEDURE AddRoute(s: Server;
                     method, path: ARRAY OF CHAR;
                     handler: HandlerProc;
                     ctx: ADDRESS): BOOLEAN;
  VAR
    sp: ServerRecPtr;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN RETURN FALSE END;
    RETURN RouterAddRoute(sp^.router, method, path, handler, ctx);
  END AddRoute;

  (* ── AddMiddleware ───────────────────────────────────── *)

  PROCEDURE AddMiddleware(s: Server;
                          mw: MiddlewareProc;
                          ctx: ADDRESS): BOOLEAN;
  VAR
    sp: ServerRecPtr;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN RETURN FALSE END;
    RETURN MwChainAdd(sp^.middleware, mw, ctx);
  END AddMiddleware;

  (* ── Connection cleanup callback ─────────────────────── *)

  (* Called by ConnOnEvent when a connection is detected as closed.
     Unwatches the fd, closes the connection, and frees the slot. *)
  PROCEDURE DoCleanup(serverAddr: ADDRESS; cp: ConnPtr);
  VAR
    sp: ServerRecPtr;
    i: CARDINAL;
    clientFd: INTEGER;
  BEGIN
    sp := ServerRecPtr(serverAddr);
    IF sp = NIL THEN RETURN END;
    IF cp = NIL THEN RETURN END;

    (* Capture fd before ConnClose, which may DEALLOCATE cp *)
    clientFd := cp^.fd;

    (* Unwatch from event loop *)
    IF cp^.watching THEN
      UnwatchFd(sp^.loop, cp^.fd);
      cp^.watching := FALSE;
    END;

    (* Find and clear the slot *)
    FOR i := 0 TO MaxServerConns - 1 DO
      IF sp^.conns[i] = cp THEN
        ConnClose(cp);
        (* Release the socket fd — ConnClose tears down TLS and
           buffers but does NOT close(2) the underlying fd, so
           without this the kernel-side TCP reaches CLOSED state
           while the userland fd leaks until ulimit -n is hit. *)
        IF clientFd >= 0 THEN
          CloseSocket(Socket(clientFd));
        END;
        sp^.conns[i] := NIL;
        IF sp^.numConns > 0 THEN
          DEC(sp^.numConns);
        END;
        DecConnsActive(sp^.metrics);
        IncConnsClosed(sp^.metrics);
        RETURN;
      END;
    END;
  END DoCleanup;

  (* ── Handshake timeout ───────────────────────────────── *)

  CONST
    HsTimeoutMs = 10000;   (* 10 seconds for H2 preface + SETTINGS *)

  (* Timer callback: if the connection is still in CpPreface or CpSettings
     after HsTimeoutMs, force-close it to reclaim the connection slot. *)
  PROCEDURE HsTimeoutCb(user: ADDRESS);
  VAR
    cp: ConnPtr;
  BEGIN
    cp := ConnPtr(user);
    IF cp = NIL THEN RETURN END;
    IF (cp^.phase = CpHandshaking) OR (cp^.phase = CpPreface)
       OR (cp^.phase = CpSettings) THEN
      cp^.hsTimerId := -1;
      cp^.phase := CpClosed;
      DoCleanup(cp^.server, cp);
    END;
  END HsTimeoutCb;

  (* ── Accept callback ─────────────────────────────────── *)

  PROCEDURE OnAccept(fd, events: INTEGER; user: ADDRESS);
  VAR
    sp: ServerRecPtr;
    clientFd: Socket;
    peer: SockAddr;
    sockSt: Sockets.Status;
    cp: ConnPtr;
    idx: CARDINAL;
    tlsSess: TLSSession;
    tlsSt: TLS.Status;
    sched: Scheduler.Scheduler;
    hsTimer: TimerId;
    tmrSt: EventLoop.Status;
  BEGIN
    sp := ServerRecPtr(user);
    IF sp = NIL THEN RETURN END;
    IF sp^.draining THEN RETURN END;

    (* Loop to accept ALL pending connections from the backlog.
       With EV_CLEAR (edge-triggered) kqueue, this callback fires
       once per state change.  If multiple connections arrive before
       we run, we must accept them all now — kevent will NOT fire
       again for connections already in the backlog. *)
    LOOP
      sockSt := Accept(Socket(fd), clientFd, peer);
      IF sockSt # Sockets.OK THEN
        EXIT;
      END;

      IncConnsAccepted(sp^.metrics);

      (* Find a free slot within the runtime connLimit *)
      idx := 0;
      WHILE (idx < sp^.connLimit) AND (sp^.conns[idx] # NIL) DO
        INC(idx);
      END;
      IF idx >= sp^.connLimit THEN
        (* At capacity — refuse at TCP level *)
        CloseSocket(clientFd);
        IncConnsRejected(sp^.metrics);
        LogProtocol(sp^.lg, 0, "reject", "conn limit reached");
        (* Continue draining to prevent backlog buildup *)
        LOOP
          sockSt := Accept(Socket(fd), clientFd, peer);
          IF sockSt # Sockets.OK THEN
            EXIT;
          END;
          CloseSocket(clientFd);
          IncConnsAccepted(sp^.metrics);
          IncConnsRejected(sp^.metrics);
        END;
        RETURN;
      END;

      (* Non-blocking TLS handshake — socket stays non-blocking.
         macOS inherits O_NONBLOCK from the listen socket. *)
      sched := GetScheduler(sp^.loop);
      tlsSt := SessionCreateServer(sp^.loop, sched,
                                   sp^.tlsCtx, INTEGER(clientFd),
                                   tlsSess);
      IF tlsSt # TLS.OK THEN
        CloseSocket(clientFd);
        (* Continue accepting remaining connections *)
      ELSE
        (* Attempt handshake — may complete or return WantRead/WantWrite *)
        tlsSt := Handshake(tlsSess);
        IF (tlsSt # TLS.OK) AND (tlsSt # TLS.WantRead)
           AND (tlsSt # TLS.WantWrite) THEN
          (* TLS handshake failed *)
          SessionDestroy(tlsSess);
          CloseSocket(clientFd);
          (* Continue accepting remaining connections *)
        ELSE
          (* Handshake completed or in progress — create connection *)
          IF ConnCreate(ADDRESS(sp), sp^.nextConnId,
                        INTEGER(clientFd), peer, cp) THEN
            cp^.tlsSess := tlsSess;
            cp^.loop := sp^.loop;
            cp^.localSettings.maxConcurrentStreams := sp^.streamLimit;
            sp^.conns[idx] := cp;
            INC(sp^.numConns);
            INC(sp^.nextConnId);
            IncConnsActive(sp^.metrics);
            LogConn(sp^.lg, cp^.id, "accepted");

            IF tlsSt = TLS.OK THEN
              (* Handshake completed synchronously *)
              WatchFd(sp^.loop, INTEGER(clientFd), EvRead,
                      ConnOnEvent, ADDRESS(cp));
            ELSE
              (* Handshake needs more I/O — watch for read+write *)
              cp^.phase := CpHandshaking;
              WatchFd(sp^.loop, INTEGER(clientFd), EvRead + EvWrite,
                      ConnOnEvent, ADDRESS(cp));
            END;
            cp^.watching := TRUE;

            tmrSt := SetTimeout(sp^.loop, HsTimeoutMs,
                                 HsTimeoutCb, ADDRESS(cp), hsTimer);
            IF tmrSt = EventLoop.OK THEN
              cp^.hsTimerId := hsTimer;
            END;
          ELSE
            SessionDestroy(tlsSess);
            CloseSocket(clientFd);
          END;
        END;
      END;
    END; (* LOOP *)
  END OnAccept;

  (* ── Start ───────────────────────────────────────────── *)

  PROCEDURE Start(s: Server): Status;
  VAR
    sp: ServerRecPtr;
    loopSt: EventLoop.Status;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN RETURN Invalid END;

    (* Watch listen socket for incoming connections *)
    loopSt := WatchFd(sp^.loop, INTEGER(sp^.listenSock), EvRead,
                      OnAccept, ADDRESS(sp));
    IF loopSt # EventLoop.OK THEN
      RETURN SysError;
    END;

    sp^.running := TRUE;
    LogProtocol(sp^.lg, 0, "start", "server listening");

    (* Enter the event loop — blocks until Stop *)
    Run(sp^.loop);

    sp^.running := FALSE;
    RETURN OK;
  END Start;

  (* ── Drain ───────────────────────────────────────────── *)

  PROCEDURE Drain(s: Server): Status;
  VAR
    sp: ServerRecPtr;
    i: CARDINAL;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN RETURN Invalid END;

    sp^.draining := TRUE;

    (* Send GOAWAY to all active connections *)
    FOR i := 0 TO MaxServerConns - 1 DO
      IF sp^.conns[i] # NIL THEN
        ConnDrain(sp^.conns[i]);
        ConnFlush(sp^.conns[i]);
      END;
    END;

    LogProtocol(sp^.lg, 0, "drain", "goaway sent to all");

    (* Stop accepting *)
    UnwatchFd(sp^.loop, INTEGER(sp^.listenSock));

    (* Stop the event loop *)
    Stop(sp^.loop);

    RETURN OK;
  END Drain;

  (* ── Stop ────────────────────────────────────────────── *)

  PROCEDURE Stop(s: Server): Status;
  VAR
    sp: ServerRecPtr;
    i: CARDINAL;
    clientFd: INTEGER;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN RETURN Invalid END;

    (* Close all connections *)
    FOR i := 0 TO MaxServerConns - 1 DO
      IF sp^.conns[i] # NIL THEN
        IF sp^.conns[i]^.watching THEN
          UnwatchFd(sp^.loop, sp^.conns[i]^.fd);
        END;
        clientFd := sp^.conns[i]^.fd;
        ConnClose(sp^.conns[i]);
        IF clientFd >= 0 THEN
          CloseSocket(Socket(clientFd));
        END;
        sp^.conns[i] := NIL;
        DecConnsActive(sp^.metrics);
        IncConnsClosed(sp^.metrics);
      END;
    END;
    sp^.numConns := 0;

    (* Stop event loop *)
    EventLoop.Stop(sp^.loop);

    LogProtocol(sp^.lg, 0, "stop", "server stopped");

    RETURN OK;
  END Stop;

  (* ── Destroy ─────────────────────────────────────────── *)

  PROCEDURE Destroy(VAR s: Server): Status;
  VAR
    sp: ServerRecPtr;
    i: CARDINAL;
    dummy: Status;
    clientFd: INTEGER;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN
      RETURN Invalid;
    END;

    (* Close any remaining connections *)
    FOR i := 0 TO MaxServerConns - 1 DO
      IF sp^.conns[i] # NIL THEN
        clientFd := sp^.conns[i]^.fd;
        ConnClose(sp^.conns[i]);
        IF clientFd >= 0 THEN
          CloseSocket(Socket(clientFd));
        END;
        sp^.conns[i] := NIL;
      END;
    END;

    (* Close listen socket *)
    IF sp^.listenSock # InvalidSocket THEN
      CloseSocket(sp^.listenSock);
      sp^.listenSock := InvalidSocket;
    END;

    (* Destroy TLS context *)
    IF sp^.tlsCtx # NIL THEN
      ContextDestroy(sp^.tlsCtx);
    END;

    (* Destroy event loop *)
    IF sp^.loop # NIL THEN
      LoopDestroy(sp^.loop);
    END;

    DEALLOCATE(sp, TSIZE(ServerRec));
    s := NIL;

    RETURN OK;
  END Destroy;

  PROCEDURE GetLoop(s: Server): ADDRESS;
  VAR
    sp: ServerRecPtr;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN RETURN NIL END;
    RETURN sp^.loop;
  END GetLoop;

  PROCEDURE SetNextConnId(s: Server; startId: CARDINAL);
  VAR sp: ServerRecPtr;
  BEGIN
    sp := ServerRecPtr(s);
    IF sp # NIL THEN sp^.nextConnId := startId END
  END SetNextConnId;

BEGIN
  (* ALPN wire format: length-prefixed "h2" *)
  alpnH2[0] := CHR(2);
  alpnH2[1] := "h";
  alpnH2[2] := "2";
END Http2Server.
