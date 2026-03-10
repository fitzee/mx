# Stream Usage Examples

Four examples demonstrating Stream in different scenarios: async TCP echo client, async HTTPS GET over TLS, sync integration with HTTPClient, and async TCP echo server.

## Example 1: TCP Echo Client (Async API)

A TCP client that connects to an echo server, sends a message, reads the response, and closes the connection. Uses the async API with Futures.

### Build

```bash
mx --m2plus examples/networking/echo_client.mod \
  -I libs/m2stream/src \
  -I libs/m2evloop/src \
  -I libs/m2futures/src \
  -I libs/m2sockets/src \
  -I libs/m2tls/src \
  libs/m2evloop/src/poller_bridge.c \
  libs/m2sockets/src/sockets_bridge.c \
  libs/m2tls/src/tls_bridge.c \
  -lssl -lcrypto
```

### Code

```modula2
MODULE EchoClient;

FROM SYSTEM IMPORT ADR;
FROM Sockets IMPORT Socket, AF_INET, SOCK_STREAM, OK,
                    SocketCreate, SetNonBlocking, Connect, CloseSocket;
FROM EventLoop IMPORT Loop, Create, GetScheduler, Run, SetInterval, Stop;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Result, GetResultIfSettled;
IMPORT Promise;
IMPORT Stream;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  loop: Loop;
  sched: Scheduler;
  sock: Socket;
  strm: Stream.Stream;
  sendFut, recvFut, closeFut: Future;
  st: Stream.Status;
  est: EventLoop.Status;
  sst: Sockets.Status;
  tid: INTEGER;
  phase: INTEGER;
  msg: ARRAY [0..63] OF CHAR;
  buf: ARRAY [0..255] OF CHAR;

PROCEDURE OnCheck(user: ADDRESS);
VAR
  settled: BOOLEAN;
  res: Result;
  pst: Promise.Status;
BEGIN
  CASE phase OF
    0: (* Wait for WriteAllAsync to complete *)
       pst := GetResultIfSettled(sendFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("Sent "); WriteInt(res.v.tag, 0);
       WriteString(" bytes"); WriteLn;
       (* Now read the echo response *)
       st := Stream.ReadAsync(strm, ADR(buf), 256, recvFut);
       IF st # Stream.OK THEN
         WriteString("ReadAsync failed"); WriteLn;
         Stop(loop); RETURN
       END;
       phase := 1 |

    1: (* Wait for ReadAsync to complete *)
       pst := GetResultIfSettled(recvFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("Received "); WriteInt(res.v.tag, 0);
       WriteString(" bytes: ");
       buf[res.v.tag] := 0C;  (* NUL-terminate *)
       WriteString(buf); WriteLn;
       (* Close the stream gracefully *)
       st := Stream.CloseAsync(strm, closeFut);
       IF st # Stream.OK THEN
         WriteString("CloseAsync failed"); WriteLn;
         Stop(loop); RETURN
       END;
       phase := 2 |

    2: (* Wait for CloseAsync to complete *)
       pst := GetResultIfSettled(closeFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("Connection closed"); WriteLn;
       Stop(loop)
  ELSE
  END
END OnCheck;

BEGIN
  phase := 0;
  msg := "Hello, echo server!";

  (* Create event loop *)
  est := Create(loop);
  sched := GetScheduler(loop);

  (* Create and connect a TCP socket *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, sock);
  sst := Connect(sock, "127.0.0.1", 9000);
  sst := SetNonBlocking(sock, TRUE);

  (* Wrap in a Stream *)
  st := Stream.CreateTCP(loop, sched, INTEGER(sock), strm);
  IF st # Stream.OK THEN
    WriteString("CreateTCP failed"); WriteLn; HALT
  END;

  (* Start sending *)
  st := Stream.WriteAllAsync(strm, ADR(msg), 19, sendFut);
  IF st # Stream.OK THEN
    WriteString("WriteAllAsync failed"); WriteLn; HALT
  END;

  (* Poll for completion *)
  est := SetInterval(loop, 10, OnCheck, NIL, tid);
  Run(loop);

  (* Cleanup *)
  st := Stream.Destroy(strm)
END EchoClient.
```

### Key Patterns

- **WriteAllAsync for complete sends**: Unlike `WriteAsync` (which may perform a partial write), `WriteAllAsync` loops internally until all bytes are sent.
- **Sequential operations via phases**: Only one async operation is pending at a time. The timer callback advances through phases as each Future settles.
- **CloseAsync for graceful shutdown**: Closes the socket through the event loop. For TLS streams, this also sends `close_notify`.

## Example 2: HTTPS GET (Async API + TLS)

An HTTPS GET client that creates a TLS stream, sends an HTTP request, reads the response, and closes. Demonstrates TLS integration with Stream.

### Build

```bash
mx --m2plus examples/networking/stream_https.mod \
  -I libs/m2stream/src \
  -I libs/m2tls/src \
  -I libs/m2evloop/src \
  -I libs/m2futures/src \
  -I libs/m2sockets/src \
  -I libs/m2http/src \
  libs/m2tls/src/tls_bridge.c \
  libs/m2evloop/src/poller_bridge.c \
  libs/m2sockets/src/sockets_bridge.c \
  libs/m2http/src/dns_bridge.c \
  -lssl -lcrypto
```

### Code

```modula2
MODULE StreamHTTPS;

FROM SYSTEM IMPORT ADR;
FROM Sockets IMPORT Socket, AF_INET, SOCK_STREAM,
                    SocketCreate, SetNonBlocking, Connect;
FROM EventLoop IMPORT Loop, Create, GetScheduler, Run, SetInterval, Stop;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Result, GetResultIfSettled;
IMPORT Promise;
IMPORT TLS;
IMPORT Stream;
FROM DNS IMPORT AddrRec, ResolveA;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  loop: Loop;
  sched: Scheduler;
  sock: Socket;
  strm: Stream.Stream;
  tlsCtx: TLS.TLSContext;
  tlsSess: TLS.TLSSession;
  hsFut, sendFut, recvFut: Future;
  tst: TLS.Status;
  st: Stream.Status;
  sst: Sockets.Status;
  est: EventLoop.Status;
  addr: AddrRec;
  tid, phase: INTEGER;
  req: ARRAY [0..255] OF CHAR;
  buf: ARRAY [0..4095] OF CHAR;
  host: ARRAY [0..63] OF CHAR;

PROCEDURE OnCheck(user: ADDRESS);
VAR settled: BOOLEAN; res: Result; pst: Promise.Status;
BEGIN
  CASE phase OF
    0: (* Wait for TLS handshake *)
       pst := GetResultIfSettled(hsFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("TLS handshake complete"); WriteLn;
       (* Create TLS stream — ownership of tlsCtx/tlsSess transfers *)
       st := Stream.CreateTLS(loop, sched, INTEGER(sock),
                               tlsCtx, tlsSess, strm);
       IF st # Stream.OK THEN
         WriteString("CreateTLS failed"); WriteLn;
         Stop(loop); RETURN
       END;
       (* Send HTTP request *)
       req := "GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n";
       st := Stream.WriteAllAsync(strm, ADR(req), 57, sendFut);
       phase := 1 |

    1: (* Wait for send *)
       pst := GetResultIfSettled(sendFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("Request sent"); WriteLn;
       st := Stream.ReadAsync(strm, ADR(buf), 4096, recvFut);
       phase := 2 |

    2: (* Wait for recv *)
       pst := GetResultIfSettled(recvFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("Received "); WriteInt(res.v.tag, 0);
       WriteString(" bytes"); WriteLn;
       buf[res.v.tag] := 0C;
       WriteString(buf); WriteLn;
       st := Stream.Destroy(strm);
       Stop(loop)
  ELSE
  END
END OnCheck;

BEGIN
  phase := 0;
  host := "example.com";

  est := Create(loop);
  sched := GetScheduler(loop);

  (* DNS resolve *)
  IF NOT ResolveA(host, 443, addr) THEN
    WriteString("DNS failed"); WriteLn; HALT
  END;

  (* TCP connect *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, sock);
  sst := Connect(sock, host, 443);
  sst := SetNonBlocking(sock, TRUE);

  (* TLS setup *)
  tst := TLS.ContextCreate(tlsCtx);
  tst := TLS.LoadSystemRoots(tlsCtx);
  tst := TLS.SessionCreate(loop, sched, tlsCtx, INTEGER(sock), tlsSess);
  tst := TLS.SetSNI(tlsSess, host);

  (* Start handshake asynchronously *)
  tst := TLS.HandshakeAsync(tlsSess, hsFut);

  est := SetInterval(loop, 10, OnCheck, NIL, tid);
  Run(loop)
END StreamHTTPS.
```

### Key Patterns

- **TLS handshake before Stream creation**: The TLS handshake completes via `TLS.HandshakeAsync` before `CreateTLS` is called. Stream requires a completed handshake.
- **Ownership transfer**: `CreateTLS` takes ownership of `tlsCtx` and `tlsSess`. After this call, the application must not call `TLS.SessionDestroy` or `TLS.ContextDestroy` directly -- `Stream.Destroy` handles it.
- **Transport transparency**: After `CreateTLS`, all I/O uses the same `WriteAllAsync`/`ReadAsync` calls as the TCP example. The TLS encryption is invisible to the caller.

## Example 3: HTTPClient Integration (Sync API)

This example shows how HTTPClient uses Stream's sync API internally. This is not standalone application code but illustrates the sync API pattern for consumers who embed Stream in their own state machines.

### Pattern

```modula2
(* Inside HTTPClient's OnSocketEvent callback *)

PROCEDURE OnSocketEvent(fd, events: INTEGER; user: ADDRESS);
VAR
  cp: ConnPtr;
  n: INTEGER;
  st: Stream.Status;
BEGIN
  cp := user;

  CASE cp^.connState OF

    StSending:
      st := Stream.TryWrite(cp^.strm,
                              OffsetPtr(ADR(cp^.request), cp^.reqSent),
                              cp^.reqLen - cp^.reqSent, n);
      IF st = Stream.OK THEN
        cp^.reqSent := cp^.reqSent + n;
        IF cp^.reqSent >= cp^.reqLen THEN
          cp^.connState := StRecvStatus;
          (* Switch watcher to read *)
          EventLoop.ModifyFd(cp^.loop, fd, EvRead)
        END
      ELSIF st = Stream.WouldBlock THEN
        (* TLS renegotiation — Stream already adjusted watcher mask *)
      ELSE
        FailConn(cp, 2)  (* send error *)
      END |

    StRecvBody:
      st := Stream.TryRead(cp^.strm, ADR(cp^.recvBuf), MaxRecv, n);
      IF st = Stream.OK THEN
        ProcessBody(cp, n)
      ELSIF st = Stream.WouldBlock THEN
        (* TLS renegotiation — Stream already adjusted watcher mask *)
      ELSIF st = Stream.StreamClosed THEN
        SucceedConn(cp)  (* connection-close body termination *)
      ELSE
        FailConn(cp, 3)  (* recv error *)
      END

  (* ... other states ... *)
  END
END OnSocketEvent;
```

### Key Patterns

- **Caller owns the watcher**: HTTPClient registers its own `OnSocketEvent` callback with EventLoop. Stream never touches the watcher registration.
- **WouldBlock is transparent**: When `TryRead` or `TryWrite` returns `WouldBlock` (TLS renegotiation), Stream has already called `ModifyFd` to adjust the watcher mask. The caller simply returns and waits for the next event.
- **Same code path for TCP and TLS**: HTTPClient does not check `GetKind`. The `TryRead`/`TryWrite` API behaves identically for both transports, with `WouldBlock` being the only TLS-specific status that can appear.
- **Partial writes handled by caller**: `TryWrite` may write fewer bytes than requested. HTTPClient tracks `reqSent` and loops.

## Choosing Between Sync and Async

| Scenario                                    | API    | Reason                                           |
|---------------------------------------------|--------|--------------------------------------------------|
| Building a protocol state machine           | Sync   | Caller already has a watcher for the state machine. |
| Simple send/receive in application code     | Async  | No need to manage watchers manually.             |
| Integration with HTTPClient                 | Sync   | HTTPClient owns the watcher.                     |
| One-off file transfer or echo client        | Async  | Future-based flow is simpler.                    |
| Multiple concurrent streams on one loop     | Either | Each stream has its own fd and watcher.          |

## Example 4: TCP Echo Server (Async API)

A TCP server that accepts one connection, reads a message, echoes it back, and closes. Demonstrates server-side Stream usage with the async API.

### Build

```bash
mx --m2plus examples/networking/echo_server.mod \
  -I libs/m2stream/src \
  -I libs/m2evloop/src \
  -I libs/m2futures/src \
  -I libs/m2sockets/src \
  -I libs/m2tls/src \
  libs/m2evloop/src/poller_bridge.c \
  libs/m2sockets/src/sockets_bridge.c \
  libs/m2tls/src/tls_bridge.c \
  -lssl -lcrypto
```

### Code

```modula2
MODULE EchoServer;

FROM SYSTEM IMPORT ADR;
FROM Sockets IMPORT Socket, AF_INET, SOCK_STREAM, OK,
                    SocketCreate, Bind, Listen, Accept,
                    SetNonBlocking, SetReuseAddr, CloseSocket;
IMPORT Sockets;
FROM EventLoop IMPORT Loop, Create, GetScheduler, Run, SetInterval, Stop;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Result, GetResultIfSettled;
IMPORT Promise;
IMPORT Stream;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST
  Port = 9000;

VAR
  loop: Loop;
  sched: Scheduler;
  listenSock, clientSock: Socket;
  strm: Stream.Stream;
  recvFut, sendFut, closeFut: Future;
  st: Stream.Status;
  est: EventLoop.Status;
  sst: Sockets.Status;
  tid, phase: INTEGER;
  buf: ARRAY [0..4095] OF CHAR;
  peer: Sockets.SockAddr;
  bytesRead: INTEGER;

PROCEDURE OnCheck(user: ADDRESS);
VAR
  settled: BOOLEAN;
  res: Result;
  pst: Promise.Status;
BEGIN
  CASE phase OF
    0: (* Wait for ReadAsync *)
       pst := GetResultIfSettled(recvFut, settled, res);
       IF NOT settled THEN RETURN END;
       bytesRead := res.v.tag;
       WriteString("Received "); WriteInt(bytesRead, 0);
       WriteString(" bytes"); WriteLn;
       (* Echo back *)
       st := Stream.WriteAllAsync(strm, ADR(buf), bytesRead, sendFut);
       IF st # Stream.OK THEN
         WriteString("WriteAllAsync failed"); WriteLn;
         Stop(loop); RETURN
       END;
       phase := 1 |

    1: (* Wait for WriteAllAsync *)
       pst := GetResultIfSettled(sendFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("Echoed "); WriteInt(res.v.tag, 0);
       WriteString(" bytes"); WriteLn;
       (* Close *)
       st := Stream.CloseAsync(strm, closeFut);
       phase := 2 |

    2: (* Wait for CloseAsync *)
       pst := GetResultIfSettled(closeFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("Connection closed"); WriteLn;
       Stop(loop)
  ELSE
  END
END OnCheck;

BEGIN
  phase := 0;

  est := Create(loop);
  sched := GetScheduler(loop);

  (* Bind and listen *)
  sst := SocketCreate(AF_INET, SOCK_STREAM, listenSock);
  sst := SetReuseAddr(listenSock, TRUE);
  sst := Bind(listenSock, Port);
  sst := Listen(listenSock, 5);
  WriteString("Listening on port "); WriteInt(Port, 0); WriteLn;

  (* Accept one connection (blocking for simplicity) *)
  sst := Accept(listenSock, clientSock, peer);
  IF sst # Sockets.OK THEN
    WriteString("Accept failed"); WriteLn; HALT
  END;
  sst := SetNonBlocking(clientSock, TRUE);
  WriteString("Client connected"); WriteLn;

  (* Wrap accepted socket in a Stream *)
  st := Stream.CreateTCP(loop, sched, INTEGER(clientSock), strm);
  IF st # Stream.OK THEN
    WriteString("CreateTCP failed"); WriteLn; HALT
  END;

  (* Start reading *)
  st := Stream.ReadAsync(strm, ADR(buf), 4096, recvFut);
  IF st # Stream.OK THEN
    WriteString("ReadAsync failed"); WriteLn; HALT
  END;

  est := SetInterval(loop, 10, OnCheck, NIL, tid);
  Run(loop);

  (* Cleanup *)
  st := Stream.Destroy(strm);
  CloseSocket(listenSock)
END EchoServer.
```

### Key Patterns

- **Accept -> SetNonBlocking -> CreateTCP**: The accepted socket is wrapped in a Stream exactly like a client-side connected socket. Stream doesn't care which end initiated the connection.
- **Blocking accept, async I/O**: For simplicity, this example uses blocking `Accept`. A production server would use EventLoop to watch the listen socket for incoming connections.
- **Sequential phases**: Read -> WriteAll -> Close, each waiting for the previous Future to settle.
- **CloseAsync for graceful shutdown**: Ensures the socket is properly closed through the event loop.

### Testing

```bash
# Terminal 1: Start the server
./echo_server
# Output: Listening on port 9000

# Terminal 2: Connect and send data
echo "Hello, server!" | nc localhost 9000
# Output: Hello, server!
```

## See Also

- [Stream](Stream.md) -- Full API reference
- [Stream-Architecture](Stream-Architecture.md) -- Internal design
- [../m2tls/https_get_example](../m2tls/https_get_example.md) -- HTTPS GET via HTTPClient (higher-level)
- [../m2http/http_get_example](../m2http/http_get_example.md) -- HTTP GET via HTTPClient
- [../m2sockets/Sockets](../m2sockets/Sockets.md) -- Low-level socket API
