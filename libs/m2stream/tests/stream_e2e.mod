MODULE StreamE2E;

(* End-to-end test for m2stream library.
   Tests sync (TryRead/TryWrite) and async (ReadAsync/WriteAsync/
   WriteAllAsync/CloseAsync) paths over loopback TCP.

   Build:
     m2c --m2plus \
       -I libs/m2futures/src -I libs/m2evloop/src -I libs/m2stream/src \
       -I libs/m2tls/src -I libs/m2sockets/src \
       libs/m2stream/tests/stream_e2e.mod \
       libs/m2evloop/src/poller_bridge.c \
       libs/m2sockets/src/sockets_bridge.c \
       libs/m2tls/src/tls_bridge.c \
       --cflag "-I/opt/homebrew/opt/openssl@3/include" \
       --cflag "-L/opt/homebrew/opt/openssl@3/lib" \
       -lssl -lcrypto -o /tmp/stream_e2e
*)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Sockets IMPORT Socket, InvalidSocket, AF_INET, SOCK_STREAM,
                    SockAddr, SocketCreate, CloseSocket, Bind, Listen,
                    Accept, Connect, SetNonBlocking;
FROM EventLoop IMPORT Loop, Create, Destroy, GetScheduler, RunOnce;
FROM Scheduler IMPORT Scheduler;
FROM Stream IMPORT Stream, StreamKind, StreamState, Status,
                   CreateTCP, CreateTLS, TryRead, TryWrite,
                   ShutdownWrite, GetState, GetKind, GetFd,
                   ReadAsync, WriteAsync, WriteAllAsync, CloseAsync;
IMPORT Stream;
FROM Promise IMPORT Future, Fate, Result, GetFate, GetResultIfSettled;
FROM SocketsBridge IMPORT m2_send, m2_recv;

CONST
  BufSize = 256;
  TestPort = 19701;
  AsyncPort = 19702;
  AsyncPort2 = 19703;

VAR
  pass, fail, total: INTEGER;
  lp: Loop;
  sched: Scheduler;

(* ── Test helpers ───────────────────────────────── *)

PROCEDURE Check(cond: BOOLEAN; name: ARRAY OF CHAR);
BEGIN
  total := total + 1;
  IF cond THEN
    pass := pass + 1;
    WriteString("  PASS: "); WriteString(name); WriteLn
  ELSE
    fail := fail + 1;
    WriteString("  FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* Spin the event loop until the future settles or maxIter reached.
   Returns TRUE if the future settled. *)
PROCEDURE PumpUntilSettled(fut: Future; maxIter: INTEGER): BOOLEAN;
VAR
  i: INTEGER;
  fate: Fate;
  hasMore: BOOLEAN;
  st: Status;
BEGIN
  i := 0;
  LOOP
    st := GetFate(fut, fate);
    IF fate # Pending THEN RETURN TRUE END;
    IF i >= maxIter THEN RETURN FALSE END;
    hasMore := RunOnce(lp);
    i := i + 1
  END
END PumpUntilSettled;

(* ── Sync tests ─────────────────────────────────── *)

PROCEDURE TestSyncReadWrite;
VAR
  listenSock, clientSock, serverSock: Socket;
  peer: SockAddr;
  sst: Status;
  str: Stream;
  st: Status;
  sendbuf: ARRAY [0..BufSize-1] OF CHAR;
  recvbuf: ARRAY [0..BufSize-1] OF CHAR;
  got, sent, n, i: INTEGER;
BEGIN
  WriteString("--- Sync TryRead/TryWrite tests ---"); WriteLn;

  (* 1. Create listen socket *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, listenSock);
  Check(sst = OK, "create listen socket");
  IF sst # OK THEN RETURN END;

  sst := Bind(listenSock, TestPort);
  Check(sst = OK, "bind listen socket");
  IF sst # OK THEN
    sst := CloseSocket(listenSock);
    RETURN
  END;

  sst := Listen(listenSock, 4);
  Check(sst = OK, "listen");

  (* 2. Create client socket and connect *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, clientSock);
  Check(sst = OK, "create client socket");

  sst := Connect(clientSock, "127.0.0.1", TestPort);
  Check(sst = OK, "connect to loopback");
  IF sst # OK THEN
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    RETURN
  END;

  (* 3. Accept connection *)
  sst := Accept(listenSock, serverSock, peer);
  Check(sst = OK, "accept connection");
  IF sst # OK THEN
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    RETURN
  END;

  (* 4. Wrap server socket in a Stream *)
  st := CreateTCP(lp, sched, serverSock, str);
  Check(st = OK, "CreateTCP");
  IF st # OK THEN
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    sst := CloseSocket(serverSock);
    RETURN
  END;

  (* 5. Verify initial state *)
  Check(GetState(str) = Open, "initial state = Open");
  Check(GetKind(str) = TCP, "kind = TCP");
  Check(GetFd(str) = serverSock, "GetFd matches");

  (* 6. Client sends, Stream reads *)
  sendbuf := "Hello from m2stream!";
  n := m2_send(clientSock, ADR(sendbuf), 20);
  Check(n > 0, "raw send from client");

  st := TryRead(str, ADR(recvbuf), BufSize, got);
  Check(st = OK, "TryRead status = OK");
  Check(got = 20, "TryRead got 20 bytes");

  (* Verify data *)
  i := 0;
  WHILE (i < 20) AND (recvbuf[i] = sendbuf[i]) DO
    i := i + 1
  END;
  Check(i = 20, "TryRead data matches");

  (* 7. Stream writes, client receives *)
  sendbuf := "Reply from stream!!";
  st := TryWrite(str, ADR(sendbuf), 19, sent);
  Check(st = OK, "TryWrite status = OK");
  Check(sent > 0, "TryWrite sent > 0");

  n := m2_recv(clientSock, ADR(recvbuf), BufSize);
  Check(n > 0, "raw recv on client");

  i := 0;
  WHILE (i < n) AND (i < 19) AND (recvbuf[i] = sendbuf[i]) DO
    i := i + 1
  END;
  Check(i = n, "TryWrite data matches");

  (* 8. Test ShutdownWrite *)
  st := ShutdownWrite(str);
  Check(st = OK, "ShutdownWrite OK");
  Check(GetState(str) = ShutdownWr, "state = ShutdownWr");

  (* 9. TryWrite after shutdown should fail *)
  st := TryWrite(str, ADR(sendbuf), 5, sent);
  Check(st = Invalid, "TryWrite after shutdown");

  (* 10. Destroy stream *)
  st := Stream.Destroy(str);
  Check(st = OK, "Destroy OK");
  Check(str = NIL, "Destroy sets NIL");

  (* Cleanup *)
  sst := CloseSocket(clientSock);
  sst := CloseSocket(listenSock)
END TestSyncReadWrite;

(* ── Async read/write tests ────────────────────── *)

PROCEDURE TestAsyncReadWrite;
VAR
  listenSock, clientSock, serverSock: Socket;
  peer: SockAddr;
  sst: Status;
  str: Stream;
  st: Status;
  sendbuf: ARRAY [0..BufSize-1] OF CHAR;
  recvbuf: ARRAY [0..BufSize-1] OF CHAR;
  fut: Future;
  settled: BOOLEAN;
  res: Result;
  n, i: INTEGER;
BEGIN
  WriteString("--- Async ReadAsync/WriteAsync tests ---"); WriteLn;

  (* Setup loopback connection *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, listenSock);
  IF sst # OK THEN
    Check(FALSE, "async: create listen socket");
    RETURN
  END;
  sst := Bind(listenSock, AsyncPort);
  IF sst # OK THEN
    Check(FALSE, "async: bind");
    sst := CloseSocket(listenSock);
    RETURN
  END;
  sst := Listen(listenSock, 4);

  sst := SocketCreate(AF_INET, SOCK_STREAM, clientSock);
  sst := Connect(clientSock, "127.0.0.1", AsyncPort);
  IF sst # OK THEN
    Check(FALSE, "async: connect");
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    RETURN
  END;

  sst := Accept(listenSock, serverSock, peer);
  IF sst # OK THEN
    Check(FALSE, "async: accept");
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    RETURN
  END;

  (* Set server socket non-blocking for async I/O *)
  sst := SetNonBlocking(serverSock, TRUE);
  Check(sst = OK, "async: set non-blocking");

  (* Create stream *)
  st := CreateTCP(lp, sched, serverSock, str);
  Check(st = OK, "async: CreateTCP");
  IF st # OK THEN
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    sst := CloseSocket(serverSock);
    RETURN
  END;

  (* --- Test ReadAsync --- *)
  (* Client sends data first, then we read asynchronously *)
  sendbuf := "Async hello M2!";
  n := m2_send(clientSock, ADR(sendbuf), 15);
  Check(n = 15, "async: client sent 15 bytes");

  st := ReadAsync(str, ADR(recvbuf), BufSize, fut);
  Check(st = OK, "ReadAsync status = OK");

  settled := PumpUntilSettled(fut, 50);
  Check(settled, "ReadAsync future settled");

  st := GetResultIfSettled(fut, settled, res);
  Check(settled, "ReadAsync result available");
  Check(res.isOk, "ReadAsync result.isOk");
  Check(res.v.tag = 15, "ReadAsync got 15 bytes");

  (* Verify data *)
  i := 0;
  WHILE (i < 15) AND (recvbuf[i] = sendbuf[i]) DO
    i := i + 1
  END;
  Check(i = 15, "ReadAsync data matches");

  (* --- Test WriteAsync --- *)
  sendbuf := "Async reply!!";
  st := WriteAsync(str, ADR(sendbuf), 13, fut);
  Check(st = OK, "WriteAsync status = OK");

  settled := PumpUntilSettled(fut, 50);
  Check(settled, "WriteAsync future settled");

  st := GetResultIfSettled(fut, settled, res);
  Check(settled, "WriteAsync result available");
  Check(res.isOk, "WriteAsync result.isOk");
  Check(res.v.tag > 0, "WriteAsync sent > 0 bytes");

  (* Verify client receives the data *)
  n := m2_recv(clientSock, ADR(recvbuf), BufSize);
  Check(n > 0, "async: client received data");

  i := 0;
  WHILE (i < n) AND (i < 13) AND (recvbuf[i] = sendbuf[i]) DO
    i := i + 1
  END;
  Check(i = n, "WriteAsync data matches");

  (* --- Test WriteAllAsync --- *)
  (* Fill sendbuf with a known pattern *)
  i := 0;
  WHILE i < 100 DO
    sendbuf[i] := CHR(65 + (i MOD 26));   (* A-Z repeating *)
    i := i + 1
  END;

  st := WriteAllAsync(str, ADR(sendbuf), 100, fut);
  Check(st = OK, "WriteAllAsync status = OK");

  settled := PumpUntilSettled(fut, 100);
  Check(settled, "WriteAllAsync future settled");

  st := GetResultIfSettled(fut, settled, res);
  Check(settled, "WriteAllAsync result available");
  Check(res.isOk, "WriteAllAsync result.isOk");
  Check(res.v.tag = 100, "WriteAllAsync sent all 100");

  (* Client drains the 100 bytes *)
  n := 0;
  i := m2_recv(clientSock, ADR(recvbuf), BufSize);
  WHILE i > 0 DO
    n := n + i;
    IF n < 100 THEN
      i := m2_recv(clientSock, ADR(recvbuf), BufSize)
    ELSE
      i := 0
    END
  END;
  Check(n = 100, "WriteAllAsync client got 100");

  (* Cleanup this stream before close test *)
  st := Stream.Destroy(str);
  sst := CloseSocket(clientSock);
  sst := CloseSocket(listenSock)
END TestAsyncReadWrite;

(* ── Async close test ──────────────────────────── *)

PROCEDURE TestAsyncClose;
VAR
  listenSock, clientSock, serverSock: Socket;
  peer: SockAddr;
  sst: Status;
  str: Stream;
  st: Status;
  fut: Future;
  settled: BOOLEAN;
  res: Result;
BEGIN
  WriteString("--- Async CloseAsync test ---"); WriteLn;

  (* Setup fresh loopback connection *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, listenSock);
  IF sst # OK THEN
    Check(FALSE, "close: create listen socket");
    RETURN
  END;
  sst := Bind(listenSock, AsyncPort2);
  IF sst # OK THEN
    Check(FALSE, "close: bind");
    sst := CloseSocket(listenSock);
    RETURN
  END;
  sst := Listen(listenSock, 4);

  sst := SocketCreate(AF_INET, SOCK_STREAM, clientSock);
  sst := Connect(clientSock, "127.0.0.1", AsyncPort2);
  IF sst # OK THEN
    Check(FALSE, "close: connect");
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    RETURN
  END;

  sst := Accept(listenSock, serverSock, peer);
  IF sst # OK THEN
    Check(FALSE, "close: accept");
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    RETURN
  END;

  sst := SetNonBlocking(serverSock, TRUE);

  st := CreateTCP(lp, sched, serverSock, str);
  Check(st = OK, "close: CreateTCP");
  IF st # OK THEN
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    sst := CloseSocket(serverSock);
    RETURN
  END;

  (* CloseAsync should close the socket and resolve *)
  st := CloseAsync(str, fut);
  Check(st = OK, "CloseAsync status = OK");

  settled := PumpUntilSettled(fut, 50);
  Check(settled, "CloseAsync future settled");

  st := GetResultIfSettled(fut, settled, res);
  Check(settled, "CloseAsync result available");
  Check(res.isOk, "CloseAsync result.isOk");
  Check(res.v.tag = 0, "CloseAsync tag = 0");

  (* Stream state should be Closed *)
  Check(GetState(str) = Closed, "CloseAsync state = Closed");

  (* fd should be invalidated *)
  Check(GetFd(str) = InvalidSocket, "CloseAsync fd = Invalid");

  (* Destroy after close should still work *)
  st := Stream.Destroy(str);
  Check(st = OK, "Destroy after CloseAsync");

  sst := CloseSocket(clientSock);
  sst := CloseSocket(listenSock)
END TestAsyncClose;

(* ── Edge case tests ────────────────────────────── *)

PROCEDURE TestEdgeCases;
VAR
  str: Stream;
  st: Status;
  dummy: INTEGER;
  buf: ARRAY [0..31] OF CHAR;
BEGIN
  WriteString("--- Edge case tests ---"); WriteLn;

  (* NIL stream operations *)
  st := TryRead(NIL, ADR(buf), 32, dummy);
  Check(st = Invalid, "TryRead NIL = Invalid");

  st := TryWrite(NIL, ADR(buf), 5, dummy);
  Check(st = Invalid, "TryWrite NIL = Invalid");

  st := ShutdownWrite(NIL);
  Check(st = Invalid, "ShutdownWrite NIL");

  Check(GetState(NIL) = Error, "GetState NIL = Error");
  Check(GetFd(NIL) = InvalidSocket, "GetFd NIL");
  Check(GetKind(NIL) = TCP, "GetKind NIL = TCP");

  str := NIL;
  st := Stream.Destroy(str);
  Check(st = Invalid, "Destroy NIL = Invalid");

  (* CreateTCP with invalid args *)
  st := CreateTCP(NIL, NIL, -1, str);
  Check(st = Invalid, "CreateTCP nil loop");

  st := CreateTCP(lp, NIL, -1, str);
  Check(st = Invalid, "CreateTCP nil sched");

  st := CreateTCP(lp, sched, -1, str);
  Check(st = Invalid, "CreateTCP bad fd");

  (* CreateTLS with invalid args *)
  st := CreateTLS(NIL, NIL, -1, NIL, NIL, str);
  Check(st = Invalid, "CreateTLS all nil")
END TestEdgeCases;

(* ── Async edge cases ──────────────────────────── *)

PROCEDURE TestAsyncEdgeCases;
VAR
  st: Status;
  fut: Future;
  buf: ARRAY [0..31] OF CHAR;
BEGIN
  WriteString("--- Async edge case tests ---"); WriteLn;

  (* ReadAsync on NIL *)
  st := ReadAsync(NIL, ADR(buf), 32, fut);
  Check(st = Invalid, "ReadAsync NIL = Invalid");

  (* WriteAsync on NIL *)
  st := WriteAsync(NIL, ADR(buf), 5, fut);
  Check(st = Invalid, "WriteAsync NIL = Invalid");

  (* WriteAllAsync on NIL *)
  st := WriteAllAsync(NIL, ADR(buf), 5, fut);
  Check(st = Invalid, "WriteAllAsync NIL = Invalid");

  (* CloseAsync on NIL *)
  st := CloseAsync(NIL, fut);
  Check(st = Invalid, "CloseAsync NIL = Invalid")
END TestAsyncEdgeCases;

(* ── Main ───────────────────────────────────────── *)

VAR
  est: Status;

BEGIN
  pass := 0;
  fail := 0;
  total := 0;

  WriteString("=== m2stream End-to-End Tests ==="); WriteLn;

  (* Create event loop (includes scheduler) *)
  est := Create(lp);
  IF est # OK THEN
    WriteString("FATAL: cannot create event loop"); WriteLn;
    HALT
  END;
  sched := GetScheduler(lp);

  TestEdgeCases;
  TestAsyncEdgeCases;
  TestSyncReadWrite;
  TestAsyncReadWrite;
  TestAsyncClose;

  est := Destroy(lp);

  WriteLn;
  WriteString("=== Results: ");
  WriteInt(pass, 0); WriteString(" passed, ");
  WriteInt(fail, 0); WriteString(" failed / ");
  WriteInt(total, 0); WriteString(" total ==="); WriteLn;

  IF fail > 0 THEN HALT END
END StreamE2E.
