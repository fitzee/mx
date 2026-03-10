# HTTPS Server Example

A TLS echo server that accepts connections, performs TLS handshake with ALPN, reads data, echoes it back, and closes. Demonstrates server-side TLS with the m2tls library.

## Overview

This example shows the server-side TLS pattern:

1. Create a server TLS context with `ContextCreateServer`
2. Load the server certificate and key with `SetServerCert`
3. Optionally configure ALPN with `SetALPNServer`
4. Bind and listen on a TCP socket
5. For each accepted connection: create a server session, handshake, read/write, close

## Build

```bash
mx --m2plus examples/networking/tls_echo_server.mod \
  -I libs/m2tls/src \
  -I libs/m2evloop/src \
  -I libs/m2futures/src \
  -I libs/m2sockets/src \
  libs/m2tls/src/tls_bridge.c \
  libs/m2evloop/src/poller_bridge.c \
  libs/m2sockets/src/sockets_bridge.c \
  -lssl -lcrypto
```

## Code

```modula2
MODULE TLSEchoServer;

FROM SYSTEM IMPORT ADR;
FROM Sockets IMPORT Socket, AF_INET, SOCK_STREAM,
                    SocketCreate, Bind, Listen, Accept,
                    SetNonBlocking, SetReuseAddr, CloseSocket;
IMPORT Sockets;
FROM EventLoop IMPORT Loop, Create, GetScheduler, Run, Stop;
IMPORT EventLoop;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Result, GetResultIfSettled;
IMPORT Promise;
IMPORT TLS;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST
  Port = 8443;

VAR
  loop: Loop;
  sched: Scheduler;
  listenSock, clientSock: Socket;
  tlsCtx: TLS.TLSContext;
  tlsSess: TLS.TLSSession;
  hsFut: Future;
  tst: TLS.Status;
  sst: Sockets.Status;
  est: EventLoop.Status;
  tid, phase, got: INTEGER;
  buf: ARRAY [0..4095] OF CHAR;
  certPath: ARRAY [0..255] OF CHAR;
  keyPath: ARRAY [0..255] OF CHAR;
  alpnProtos: ARRAY [0..2] OF CHAR;
  alpnBuf: ARRAY [0..15] OF CHAR;
  alpnLen: INTEGER;
  peer: Sockets.SockAddr;

PROCEDURE OnCheck(user: ADDRESS);
VAR
  settled: BOOLEAN;
  res: Result;
  pst: Promise.Status;
  sent: INTEGER;
BEGIN
  CASE phase OF
    0: (* Wait for TLS handshake *)
       pst := GetResultIfSettled(hsFut, settled, res);
       IF NOT settled THEN RETURN END;
       WriteString("TLS handshake complete"); WriteLn;
       (* Check ALPN *)
       tst := TLS.GetALPN(tlsSess, alpnBuf, alpnLen);
       IF alpnLen > 0 THEN
         WriteString("ALPN negotiated: "); WriteString(alpnBuf); WriteLn
       END;
       (* Read data from client *)
       tst := TLS.Read(tlsSess, ADR(buf), 4096, got);
       IF tst = TLS.OK THEN
         phase := 1
       ELSIF tst = TLS.WantRead THEN
         RETURN  (* retry on next tick *)
       ELSE
         WriteString("Read error"); WriteLn;
         Stop(loop); RETURN
       END |

    1: (* Echo data back *)
       tst := TLS.Write(tlsSess, ADR(buf), got, sent);
       IF tst = TLS.OK THEN
         WriteString("Echoed "); WriteInt(sent, 0);
         WriteString(" bytes"); WriteLn;
         tst := TLS.Shutdown(tlsSess);
         tst := TLS.SessionDestroy(tlsSess);
         CloseSocket(clientSock);
         Stop(loop)
       ELSIF tst = TLS.WantWrite THEN
         RETURN  (* retry on next tick *)
       ELSE
         WriteString("Write error"); WriteLn;
         Stop(loop)
       END
  ELSE
  END
END OnCheck;

BEGIN
  phase := 0;
  certPath := "server.pem";
  keyPath := "server-key.pem";

  (* ALPN wire format: \002h2 *)
  alpnProtos[0] := CHR(2);
  alpnProtos[1] := "h";
  alpnProtos[2] := "2";

  (* Create event loop *)
  est := Create(loop);
  sched := GetScheduler(loop);

  (* Create server TLS context *)
  tst := TLS.ContextCreateServer(tlsCtx);
  IF tst # TLS.OK THEN
    WriteString("ContextCreateServer failed"); WriteLn; HALT
  END;
  tst := TLS.SetServerCert(tlsCtx, certPath, keyPath);
  IF tst # TLS.OK THEN
    WriteString("SetServerCert failed"); WriteLn; HALT
  END;
  tst := TLS.SetALPNServer(tlsCtx, ADR(alpnProtos), 3);

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

  (* Create server TLS session *)
  tst := TLS.SessionCreateServer(loop, sched, tlsCtx,
                                   INTEGER(clientSock), tlsSess);
  IF tst # TLS.OK THEN
    WriteString("SessionCreateServer failed"); WriteLn; HALT
  END;

  (* Start async handshake *)
  tst := TLS.HandshakeAsync(tlsSess, hsFut);
  IF tst # TLS.OK THEN
    WriteString("HandshakeAsync failed"); WriteLn; HALT
  END;

  est := EventLoop.SetInterval(loop, 10, OnCheck, NIL, tid);
  Run(loop);

  (* Cleanup *)
  tst := TLS.ContextDestroy(tlsCtx);
  CloseSocket(listenSock)
END TLSEchoServer.
```

## Key Patterns

- **ContextCreateServer vs ContextCreate**: Server contexts use `TLS_server_method()` internally and default to `NoVerify` (servers don't verify client certificates by default).
- **SetServerCert is required**: Unlike client contexts where certificates are optional, servers must load a certificate and private key.
- **SessionCreateServer vs SessionCreate**: Server sessions use `SSL_set_accept_state()` -- they wait for the client to initiate the TLS handshake.
- **ALPN is optional but recommended for HTTP/2**: The wire format is length-prefixed: `\002h2` means "2 bytes: h2". After handshake, `GetALPN` reports what was negotiated.
- **Shared context, per-connection sessions**: One `TLSContext` serves all connections. Each accepted socket gets its own `TLSSession`.

## Generating Test Certificates

For local testing, generate a self-signed certificate:

```bash
openssl req -x509 -newkey rsa:2048 -keyout server-key.pem -out server.pem \
  -days 365 -nodes -subj "/CN=localhost"
```

## Testing with curl

```bash
# Start the server, then in another terminal:
curl -k https://localhost:8443/ -d "Hello from curl"
# -k skips certificate verification (self-signed cert)
```

## See Also

- [TLS](TLS.md) -- Full API reference
- [TLS-Architecture](TLS-Architecture.md) -- Internal design
- [https_get_example](https_get_example.md) -- Client-side HTTPS GET example
- [../m2stream/stream_usage_example](../m2stream/stream_usage_example.md) -- Stream usage examples
