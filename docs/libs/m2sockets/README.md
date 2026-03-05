# m2sockets

## Why

Minimal POSIX/BSD sockets wrapper for Modula-2+, targeting Linux and macOS. Provides TCP and UDP socket operations with a consistent Status-based error model, including datagram I/O (SendTo/RecvFrom), multicast group management, and non-blocking mode.

## Types

- **Socket** -- INTEGER file descriptor for the underlying OS socket.
- **SockAddr** -- IPv4 socket address: family (AF_INET), port (host byte order), addrV4 (4-byte array, e.g. 127,0,0,1).
- **Status** -- `(OK, WouldBlock, Closed, SysError, Invalid)`.
  - OK -- success.
  - WouldBlock -- non-blocking socket has no data (EAGAIN/EWOULDBLOCK).
  - Closed -- peer closed the connection.
  - SysError -- OS error; call GetLastErrno()/GetLastErrorText() for details.
  - Invalid -- bad argument (programmer error).

## Constants

| Constant | Value | Description |
|----------|-------|-------------|
| InvalidSocket | -1 | Sentinel for uninitialized/failed socket |
| AF_INET | 2 | IPv4 address family |
| SOCK_STREAM | 1 | TCP (stream) socket type |
| SOCK_DGRAM | 2 | UDP (datagram) socket type |
| SHUT_RD | 0 | Shutdown reads |
| SHUT_WR | 1 | Shutdown writes |
| SHUT_RDWR | 2 | Shutdown both directions |

## Procedures

### Lifecycle

- `SocketCreate(family, socktype: INTEGER; VAR out: Socket): Status` -- Create a socket. family = AF_INET; socktype = SOCK_STREAM or SOCK_DGRAM. On OK, out contains a valid fd.
- `CloseSocket(s: Socket): Status` -- Close a socket. Idempotent if s = InvalidSocket.
- `Shutdown(s: Socket; how: INTEGER): Status` -- Half-close. how = SHUT_RD, SHUT_WR, or SHUT_RDWR.

### Server

- `Bind(s: Socket; port: CARDINAL): Status` -- Bind to INADDR_ANY on the given port. Sets SO_REUSEADDR automatically.
- `Listen(s: Socket; backlog: INTEGER): Status` -- Mark socket as passive (listening). backlog > 0.
- `Accept(s: Socket; VAR outClient: Socket; VAR outPeer: SockAddr): Status` -- Accept one connection. Blocks unless non-blocking. outClient is the new fd; outPeer has the peer's address/port.

### Client

- `Connect(s: Socket; host: ARRAY OF CHAR; port: CARDINAL): Status` -- Resolve host (name or dotted-quad) and connect. For SOCK_STREAM sockets.

### TCP I/O

- `SendBytes(s: Socket; VAR buf: ARRAY OF BYTE; len: CARDINAL; VAR sent: CARDINAL): Status` -- Send up to len bytes. sent = actual bytes sent (may be < len).
- `RecvBytes(s: Socket; VAR buf: ARRAY OF BYTE; max: CARDINAL; VAR got: CARDINAL): Status` -- Receive up to max bytes. Returns Closed when peer disconnects.

### Convenience

- `SendString(s: Socket; str: ARRAY OF CHAR): Status` -- Send a NUL-terminated string (excluding the NUL).
- `RecvLine(s: Socket; VAR line: ARRAY OF CHAR): Status` -- Read until LF or buffer full. Strips trailing CR+LF. Returns Closed if peer disconnects before any data.

### UDP (Datagram) I/O

- `SendTo(s: Socket; VAR buf: ARRAY OF BYTE; len: CARDINAL; VAR addr: SockAddr): INTEGER` -- Send len bytes to the specified address. Returns bytes sent, or -1 on error.
- `RecvFrom(s: Socket; VAR buf: ARRAY OF BYTE; maxLen: CARDINAL; VAR addr: SockAddr): INTEGER` -- Receive up to maxLen bytes. addr is filled with the sender's address/port. Returns bytes received, 0 on peer close, or -1 on error.
- `SetMulticastGroup(s: Socket; group: ARRAY OF CHAR; join: BOOLEAN): Status` -- Join (join=TRUE) or leave (join=FALSE) an IP multicast group. group is a dotted-quad address (e.g. "239.1.2.3").
- `SetBroadcast(s: Socket; enable: BOOLEAN): Status` -- Enable or disable SO_BROADCAST on the socket.

### Non-blocking

- `SetNonBlocking(s: Socket; enable: BOOLEAN): Status` -- Set or clear O_NONBLOCK on the socket.

### Error Info

- `GetLastErrno(): INTEGER` -- Return raw errno from the last failed call.
- `GetLastErrorText(VAR out: ARRAY OF CHAR)` -- Copy strerror text into out (NUL-terminated).

## Example

### TCP Echo Server

```modula2
MODULE EchoServer;

FROM Sockets IMPORT Socket, SockAddr, Status, InvalidSocket,
                     AF_INET, SOCK_STREAM, SHUT_RDWR,
                     SocketCreate, Bind, Listen, Accept,
                     RecvBytes, SendBytes, CloseSocket;
FROM SYSTEM IMPORT BYTE;

VAR
  srv, cli: Socket;
  peer:     SockAddr;
  buf:      ARRAY [0..1023] OF BYTE;
  got, sent: CARDINAL;
  st:       Status;

BEGIN
  st := SocketCreate(AF_INET, SOCK_STREAM, srv);
  st := Bind(srv, 9000);
  st := Listen(srv, 5);

  st := Accept(srv, cli, peer);
  IF st = Status.OK THEN
    LOOP
      st := RecvBytes(cli, buf, 1024, got);
      IF (st # Status.OK) OR (got = 0) THEN EXIT END;
      st := SendBytes(cli, buf, got, sent);
      IF st # Status.OK THEN EXIT END;
    END;
    st := CloseSocket(cli);
  END;
  st := CloseSocket(srv);
END EchoServer.
```

### UDP Send/Receive

```modula2
MODULE UdpExample;

FROM Sockets IMPORT Socket, SockAddr, Status,
                     AF_INET, SOCK_DGRAM,
                     SocketCreate, Bind, SendTo, RecvFrom,
                     CloseSocket;
FROM SYSTEM IMPORT BYTE;

VAR
  s:    Socket;
  addr: SockAddr;
  buf:  ARRAY [0..511] OF BYTE;
  n:    INTEGER;
  st:   Status;

BEGIN
  st := SocketCreate(AF_INET, SOCK_DGRAM, s);
  st := Bind(s, 5000);

  (* Receive a datagram *)
  n := RecvFrom(s, buf, 512, addr);
  IF n > 0 THEN
    (* Echo it back to the sender *)
    n := SendTo(s, buf, CARDINAL(n), addr);
  END;

  st := CloseSocket(s);
END UdpExample.
```
