MODULE StreamPartial;

(* Tests Stream partial read/write behavior over loopback TCP.
   Sends data in small chunks and verifies reassembly. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Sockets IMPORT Socket, InvalidSocket, AF_INET, SOCK_STREAM,
                    SockAddr, SocketCreate, CloseSocket, Bind, Listen,
                    Accept, Connect, SetNonBlocking;
FROM EventLoop IMPORT Loop, Create, Destroy, GetScheduler;
FROM Scheduler IMPORT Scheduler;
FROM Stream IMPORT Stream, StreamKind, StreamState, Status,
                   CreateTCP, TryRead, TryWrite, ShutdownWrite;
IMPORT Stream;
FROM SocketsBridge IMPORT m2_send, m2_recv;

CONST
  Port = 19711;
  BufSize = 256;

VAR
  pass, fail, total: INTEGER;
  lp: Loop;
  sched: Scheduler;

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

PROCEDURE TestPartialWrites;
VAR
  listenSock, clientSock, serverSock: Socket;
  peer: SockAddr;
  sst: Status;
  str: Stream;
  st: Status;
  sendbuf: ARRAY [0..BufSize-1] OF CHAR;
  recvbuf: ARRAY [0..BufSize-1] OF CHAR;
  n, got, sent, totalSent, totalRecv, i: INTEGER;
BEGIN
  WriteString("--- Partial write/read tests ---"); WriteLn;

  (* Setup loopback connection *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, listenSock);
  sst := Bind(listenSock, Port);
  sst := Listen(listenSock, 4);
  sst := SocketCreate(AF_INET, SOCK_STREAM, clientSock);
  sst := Connect(clientSock, "127.0.0.1", Port);
  sst := Accept(listenSock, serverSock, peer);

  st := CreateTCP(lp, sched, serverSock, str);
  Check(st = OK, "CreateTCP");
  IF st # OK THEN
    sst := CloseSocket(listenSock);
    sst := CloseSocket(clientSock);
    sst := CloseSocket(serverSock);
    RETURN
  END;

  (* Client sends 5 small chunks, stream reads them *)
  totalRecv := 0;
  FOR i := 0 TO 4 DO
    sendbuf[0] := CHR(65 + i);  (* A, B, C, D, E *)
    sendbuf[1] := CHR(0);
    n := m2_send(clientSock, ADR(sendbuf), 1);
    Check(n = 1, "chunk send");

    st := TryRead(str, ADR(recvbuf) + totalRecv, BufSize - totalRecv, got);
    IF st = OK THEN
      totalRecv := totalRecv + got
    END
  END;
  Check(totalRecv = 5, "received all 5 chunks");

  (* Verify reassembled data *)
  Check(recvbuf[0] = "A", "chunk 0 = A");
  Check(recvbuf[4] = "E", "chunk 4 = E");

  (* Stream writes back in one shot, client reads *)
  sendbuf := "REPLY";
  st := TryWrite(str, ADR(sendbuf), 5, sent);
  Check(st = OK, "write reply");
  Check(sent = 5, "sent 5 bytes");

  n := m2_recv(clientSock, ADR(recvbuf), BufSize);
  Check(n = 5, "client got 5");
  Check(recvbuf[0] = "R", "reply[0] = R");

  (* Cleanup *)
  st := Stream.Destroy(str);
  sst := CloseSocket(clientSock);
  sst := CloseSocket(listenSock)
END TestPartialWrites;

VAR est: Status;

BEGIN
  pass := 0; fail := 0; total := 0;
  WriteString("=== Stream Partial R/W Tests ==="); WriteLn;

  est := Create(lp);
  IF est # OK THEN
    WriteString("FATAL: cannot create loop"); WriteLn;
    HALT
  END;
  sched := GetScheduler(lp);

  TestPartialWrites;

  est := Destroy(lp);
  WriteLn;
  WriteString("Results: ");
  WriteInt(pass, 0); WriteString(" passed, ");
  WriteInt(fail, 0); WriteString(" failed / ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;

  IF fail > 0 THEN HALT END
END StreamPartial.
