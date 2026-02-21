MODULE EchoServer;

(* Minimal TCP echo server using the Sockets library.
   Listens on port 7000.  Accepts one client at a time,
   reads lines, and echoes them back prefixed with "ECHO: ".
   Quit by sending a line containing just "quit". *)

FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Sockets IMPORT
     Socket, SockAddr, Status, InvalidSocket,
     AF_INET, SOCK_STREAM, SHUT_RDWR,
     SocketCreate, CloseSocket, Bind, Listen, Accept,
     RecvLine, SendString, Shutdown, GetLastErrno;

CONST
  Port = 9999;

VAR
  server, client: Socket;
  peer: SockAddr;
  st: Status;
  line: ARRAY [0..1023] OF CHAR;
  nl: ARRAY [0..1] OF CHAR;
  running: BOOLEAN;

PROCEDURE WriteAddr(VAR a: SockAddr);
BEGIN
  WriteInt(ORD(a.addrV4[0]), 1); WriteString(".");
  WriteInt(ORD(a.addrV4[1]), 1); WriteString(".");
  WriteInt(ORD(a.addrV4[2]), 1); WriteString(".");
  WriteInt(ORD(a.addrV4[3]), 1);
  WriteString(":"); WriteInt(INTEGER(a.port), 1)
END WriteAddr;

BEGIN
  nl[0] := 12C; nl[1] := 0C;  (* newline *)
  st := SocketCreate(AF_INET, SOCK_STREAM, server);
  IF st # OK THEN
    WriteString("SocketCreate failed, errno=");
    WriteInt(GetLastErrno(), 1); WriteLn;
    RETURN
  END;

  st := Bind(server, Port);
  IF st # OK THEN
    WriteString("Bind failed, errno=");
    WriteInt(GetLastErrno(), 1); WriteLn;
    CloseSocket(server); RETURN
  END;

  st := Listen(server, 8);
  IF st # OK THEN
    WriteString("Listen failed, errno=");
    WriteInt(GetLastErrno(), 1); WriteLn;
    CloseSocket(server); RETURN
  END;

  WriteString("Echo server listening on port ");
  WriteInt(Port, 1); WriteLn;

  running := TRUE;
  WHILE running DO
    WriteString("Waiting for connection..."); WriteLn;
    st := Accept(server, client, peer);
    IF st # OK THEN
      WriteString("Accept failed, errno=");
      WriteInt(GetLastErrno(), 1); WriteLn;
    ELSE
      WriteString("Client connected from ");
      WriteAddr(peer); WriteLn;

      LOOP
        st := RecvLine(client, line);
        IF st = Closed THEN
          WriteString("Client disconnected."); WriteLn;
          EXIT
        END;
        IF st # OK THEN
          WriteString("RecvLine error, errno=");
          WriteInt(GetLastErrno(), 1); WriteLn;
          EXIT
        END;

        WriteString(">> "); WriteString(line); WriteLn;

        (* Check for quit command *)
        IF (line[0] = "q") AND (line[1] = "u") AND
           (line[2] = "i") AND (line[3] = "t") AND
           (line[4] = 0C) THEN
          WriteString("Quit command received."); WriteLn;
          running := FALSE;
          EXIT
        END;

        (* Echo back *)
        st := SendString(client, "ECHO: ");
        IF st = OK THEN st := SendString(client, line) END;
        IF st = OK THEN st := SendString(client, nl) END;
        IF st # OK THEN
          WriteString("Send error."); WriteLn;
          EXIT
        END
      END;

      Shutdown(client, SHUT_RDWR);
      CloseSocket(client)
    END
  END;

  CloseSocket(server);
  WriteString("Server stopped."); WriteLn
END EchoServer.
