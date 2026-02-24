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
                               CpPreface, CpSettings, CpOpen,
                               CpGoaway, CpClosed,
                               SetServerDispatch, SetConnCleanup;
  FROM Http2ServerMetrics IMPORT Metrics, MetricsInit,
                                  IncConnsAccepted, IncConnsActive,
                                  DecConnsActive, IncConnsClosed;
  FROM Http2ServerLog IMPORT LogInit, LogConn, LogProtocol;
  FROM Log IMPORT Logger;
  FROM Sockets IMPORT Socket, SockAddr, SocketCreate, Bind, Listen,
                       Accept, CloseSocket, SetNonBlocking,
                       AF_INET, SOCK_STREAM, InvalidSocket;
  FROM EventLoop IMPORT Loop, WatchFd, UnwatchFd, Run, Stop,
                         Create AS LoopCreate,
                         Destroy AS LoopDestroy,
                         GetScheduler;
  FROM Poller IMPORT EvRead;
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

    sockSt := Bind(sp^.listenSock, opts.port);
    IF sockSt # Sockets.OK THEN
      CloseSocket(sp^.listenSock);
      ContextDestroy(sp^.tlsCtx);
      LoopDestroy(sp^.loop);
      DEALLOCATE(sp, TSIZE(ServerRec));
      RETURN SysError;
    END;

    sockSt := Listen(sp^.listenSock, 128);
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
  BEGIN
    sp := ServerRecPtr(serverAddr);
    IF sp = NIL THEN RETURN END;
    IF cp = NIL THEN RETURN END;

    (* Unwatch from event loop *)
    IF cp^.watching THEN
      UnwatchFd(sp^.loop, cp^.fd);
      cp^.watching := FALSE;
    END;

    (* Find and clear the slot *)
    FOR i := 0 TO MaxServerConns - 1 DO
      IF sp^.conns[i] = cp THEN
        ConnClose(cp);
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
  BEGIN
    sp := ServerRecPtr(user);
    IF sp = NIL THEN RETURN END;
    IF sp^.draining THEN RETURN END;

    sockSt := Accept(Socket(fd), clientFd, peer);
    IF sockSt # Sockets.OK THEN
      RETURN;
    END;

    IncConnsAccepted(sp^.metrics);

    (* Find a free slot *)
    idx := 0;
    WHILE (idx < MaxServerConns) AND (sp^.conns[idx] # NIL) DO
      INC(idx);
    END;
    IF idx >= MaxServerConns THEN
      CloseSocket(clientFd);
      RETURN;
    END;

    (* Ensure accepted socket is blocking for TLS handshake.
       macOS inherits O_NONBLOCK from the listen socket. *)
    sockSt := SetNonBlocking(clientFd, FALSE);

    (* TLS handshake — socket must be blocking *)
    sched := GetScheduler(sp^.loop);
    tlsSt := SessionCreateServer(sp^.loop, sched,
                                 sp^.tlsCtx, INTEGER(clientFd),
                                 tlsSess);
    IF tlsSt # TLS.OK THEN
      CloseSocket(clientFd);
      RETURN;
    END;

    tlsSt := Handshake(tlsSess);
    IF tlsSt # TLS.OK THEN
      SessionDestroy(tlsSess);
      CloseSocket(clientFd);
      RETURN;
    END;

    (* Set non-blocking for event-loop driven I/O *)
    sockSt := SetNonBlocking(clientFd, TRUE);

    IF ConnCreate(ADDRESS(sp), sp^.nextConnId,
                  INTEGER(clientFd), peer, cp) THEN
      cp^.tlsSess := tlsSess;
      cp^.loop := sp^.loop;
      sp^.conns[idx] := cp;
      INC(sp^.numConns);
      INC(sp^.nextConnId);
      IncConnsActive(sp^.metrics);
      LogConn(sp^.lg, cp^.id, "accepted");

      (* Watch for read events on this connection *)
      WatchFd(sp^.loop, INTEGER(clientFd), EvRead,
              ConnOnEvent, ADDRESS(cp));
      cp^.watching := TRUE;

      (* Note: TLS 1.3 may have buffered data during handshake.
         The event loop will pick it up on the next iteration. *)
    ELSE
      SessionDestroy(tlsSess);
      CloseSocket(clientFd);
    END;
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
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN RETURN Invalid END;

    (* Close all connections *)
    FOR i := 0 TO MaxServerConns - 1 DO
      IF sp^.conns[i] # NIL THEN
        IF sp^.conns[i]^.watching THEN
          UnwatchFd(sp^.loop, sp^.conns[i]^.fd);
        END;
        ConnClose(sp^.conns[i]);
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
  BEGIN
    sp := ServerRecPtr(s);
    IF sp = NIL THEN
      RETURN Invalid;
    END;

    (* Close any remaining connections *)
    FOR i := 0 TO MaxServerConns - 1 DO
      IF sp^.conns[i] # NIL THEN
        ConnClose(sp^.conns[i]);
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

BEGIN
  (* ALPN wire format: length-prefixed "h2" *)
  alpnH2[0] := CHR(2);
  alpnH2[1] := "h";
  alpnH2[2] := "2";
END Http2Server.
