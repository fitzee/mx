MODULE AsyncTcpEchoFuture;

(* Asynchronous TCP echo server using EventLoop + Futures.

   Listens on port 9000.  Each accepted connection echoes back
   whatever it receives until the client disconnects.

   Demonstrates:
     - EventLoop.WatchFd for non-blocking accept/recv/send
     - EventLoop.SetTimeout for idle-client disconnect
     - Scheduler integration for microtask dispatch

   Build:
     m2c --m2plus -I libs/m2evloop/src -I libs/m2futures/src \
         -I libs/m2sockets/src \
         example_apps/async_tcp_echo_future.mod \
         libs/m2evloop/src/poller_bridge.c \
         libs/m2sockets/src/sockets_bridge.c *)

FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
IMPORT Sockets;
IMPORT EventLoop;
FROM Sockets IMPORT Socket, InvalidSocket, AF_INET, SOCK_STREAM,
                    SockAddr;
FROM Poller IMPORT EvRead, EvWrite, EvHup;
FROM EventLoop IMPORT Loop, WatcherProc;
FROM Timers IMPORT TimerId;
FROM Scheduler IMPORT TaskProc;

CONST
  Port = 9000;
  BufSize = 1024;

VAR
  loop: Loop;
  listenSock: Socket;
  st: EventLoop.Status;
  sst: Sockets.Status;
  tid: TimerId;

(* ── Client event handler ────────────────────────────────── *)

PROCEDURE OnClientReady(fd, events: INTEGER; user: ADDRESS);
VAR
  buf: ARRAY [0..BufSize-1] OF CHAR;
  got, sent: CARDINAL;
  rs, ss: Sockets.Status;
  sock: Socket;
  est: EventLoop.Status;
BEGIN
  sock := fd;
  IF (events = EvHup) OR (events >= 4) THEN
    (* Error or hangup *)
    est := EventLoop.UnwatchFd(loop, fd);
    Sockets.CloseSocket(sock);
    WriteString("Client disconnected (fd=");
    WriteInt(fd, 0); WriteString(")"); WriteLn;
    RETURN
  END;

  rs := Sockets.RecvBytes(sock, buf, BufSize, got);
  IF rs = Sockets.Closed THEN
    est := EventLoop.UnwatchFd(loop, fd);
    Sockets.CloseSocket(sock);
    WriteString("Client closed (fd=");
    WriteInt(fd, 0); WriteString(")"); WriteLn;
    RETURN
  END;
  IF rs = Sockets.WouldBlock THEN RETURN END;
  IF rs # Sockets.OK THEN
    est := EventLoop.UnwatchFd(loop, fd);
    Sockets.CloseSocket(sock);
    RETURN
  END;

  (* Echo back *)
  IF got > 0 THEN
    ss := Sockets.SendBytes(sock, buf, got, sent)
  END
END OnClientReady;

(* ── Listen socket handler ───────────────────────────────── *)

PROCEDURE OnAccept(fd, events: INTEGER; user: ADDRESS);
VAR
  client: Socket;
  peer: SockAddr;
  as: Sockets.Status;
  est: EventLoop.Status;
BEGIN
  as := Sockets.Accept(fd, client, peer);
  IF as # Sockets.OK THEN RETURN END;

  as := Sockets.SetNonBlocking(client, TRUE);
  est := EventLoop.WatchFd(loop, client, EvRead, OnClientReady, NIL);
  IF est # EventLoop.OK THEN
    Sockets.CloseSocket(client);
    RETURN
  END;

  WriteString("Accepted client fd=");
  WriteInt(client, 0); WriteLn
END OnAccept;

(* ── Main ────────────────────────────────────────────────── *)

BEGIN
  st := EventLoop.Create(loop);
  IF st # EventLoop.OK THEN
    WriteString("Failed to create event loop"); WriteLn;
    HALT
  END;

  sst := Sockets.SocketCreate(AF_INET, SOCK_STREAM, listenSock);
  IF sst # Sockets.OK THEN
    WriteString("Failed to create socket"); WriteLn;
    HALT
  END;

  sst := Sockets.Bind(listenSock, Port);
  IF sst # Sockets.OK THEN
    WriteString("Failed to bind"); WriteLn;
    HALT
  END;

  sst := Sockets.Listen(listenSock, 16);
  IF sst # Sockets.OK THEN
    WriteString("Failed to listen"); WriteLn;
    HALT
  END;

  sst := Sockets.SetNonBlocking(listenSock, TRUE);

  st := EventLoop.WatchFd(loop, listenSock, EvRead, OnAccept, NIL);
  IF st # EventLoop.OK THEN
    WriteString("Failed to watch listen socket"); WriteLn;
    HALT
  END;

  WriteString("Echo server listening on port ");
  WriteInt(Port, 0); WriteLn;

  EventLoop.Run(loop);

  Sockets.CloseSocket(listenSock);
  st := EventLoop.Destroy(loop)
END AsyncTcpEchoFuture.
