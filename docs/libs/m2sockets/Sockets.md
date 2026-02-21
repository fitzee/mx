# Sockets

TCP and UDP networking over POSIX/BSD system calls via a thin C bridge (`sockets_bridge.c`). All procedures return a `Status` value indicating success or the category of failure. Sockets are blocking by default; call `SetNonBlocking` to enable non-blocking I/O. Targets Linux and macOS.

## Types

**`Socket`** -- `INTEGER` alias holding the underlying file descriptor. A value of `InvalidSocket` (-1) indicates no valid socket.

**`SockAddr`** -- Peer address record returned by `Accept`:

| Field    | Type                    | Description                                  |
|----------|-------------------------|----------------------------------------------|
| `family` | `INTEGER`               | Address family (`AF_INET` = 2)               |
| `port`   | `CARDINAL`              | Port number in host byte order               |
| `addrV4` | `ARRAY [0..3] OF BYTE`  | IPv4 address octets (e.g. 127, 0, 0, 1)     |

**`Status`** -- Enumeration returned by every procedure:

| Value        | Meaning                                                                 |
|--------------|-------------------------------------------------------------------------|
| `OK`         | Operation succeeded.                                                    |
| `WouldBlock` | Non-blocking socket has no data ready (maps from `EAGAIN`/`EWOULDBLOCK`). |
| `Closed`     | Peer closed the connection (recv returned 0).                           |
| `SysError`   | OS-level error; call `GetLastErrno` or `GetLastErrorText` for details.  |
| `Invalid`    | Bad argument such as `InvalidSocket` or out-of-range parameter.         |

## Constants

| Constant        | Value | Description                           |
|-----------------|-------|---------------------------------------|
| `InvalidSocket` | -1    | Sentinel for "no socket"              |
| `AF_INET`       | 2     | IPv4 address family                   |
| `SOCK_STREAM`   | 1     | TCP (stream) socket type              |
| `SOCK_DGRAM`    | 2     | UDP (datagram) socket type            |
| `SHUT_RD`       | 0     | Shutdown reading half                 |
| `SHUT_WR`       | 1     | Shutdown writing half                 |
| `SHUT_RDWR`     | 2     | Shutdown both halves                  |

## Lifecycle

### SocketCreate

```modula2
PROCEDURE SocketCreate(family, socktype: INTEGER;
                       VAR out: Socket): Status;
```

Creates a new socket. `family` must be `AF_INET`; `socktype` must be `SOCK_STREAM` or `SOCK_DGRAM`. On success, `out` receives a valid file descriptor. On failure, `out` is set to `InvalidSocket` and the return value is `Invalid` (bad arguments) or `SysError` (OS error).

```modula2
st := SocketCreate(AF_INET, SOCK_STREAM, sock);
IF st # OK THEN (* handle error *) END;
```

### CloseSocket

```modula2
PROCEDURE CloseSocket(s: Socket): Status;
```

Closes the socket file descriptor. Idempotent: passing `InvalidSocket` returns `OK` without calling the OS. Returns `SysError` if the underlying `close` call fails.

### Shutdown

```modula2
PROCEDURE Shutdown(s: Socket; how: INTEGER): Status;
```

Half-closes the socket. `how` must be one of `SHUT_RD`, `SHUT_WR`, or `SHUT_RDWR`. Returns `Invalid` if `s` is `InvalidSocket` or `how` is out of range. Returns `SysError` on OS failure.

```modula2
st := Shutdown(sock, SHUT_WR);  (* signal no more writes *)
```

## Server

### Bind

```modula2
PROCEDURE Bind(s: Socket; port: CARDINAL): Status;
```

Binds the socket to `INADDR_ANY` on the given `port`. Automatically enables `SO_REUSEADDR` before binding so that restarting a server does not fail with "address already in use." `port` should be in the range 1..65535. Returns `Invalid` if `s` is `InvalidSocket`; `SysError` on OS failure.

```modula2
st := Bind(sock, 8080);
```

### Listen

```modula2
PROCEDURE Listen(s: Socket; backlog: INTEGER): Status;
```

Marks the socket as passive (accepting connections). `backlog` specifies the maximum length of the pending connection queue. If `backlog` is less than 1, it is silently clamped to 8. The socket must already be bound. Returns `Invalid` if `s` is `InvalidSocket`.

### Accept

```modula2
PROCEDURE Accept(s: Socket;
                 VAR outClient: Socket;
                 VAR outPeer: SockAddr): Status;
```

Accepts one incoming connection. Blocks until a client connects (unless the socket is non-blocking, in which case it may return `WouldBlock`). On success, `outClient` receives the new connected file descriptor and `outPeer` is filled with the peer's IPv4 address and port. On failure, `outClient` is set to `InvalidSocket`.

```modula2
st := Accept(serverSock, clientSock, peer);
IF st = OK THEN
  (* peer.port and peer.addrV4 identify the remote end *)
END;
```

## Client

### Connect

```modula2
PROCEDURE Connect(s: Socket;
                  host: ARRAY OF CHAR;
                  port: CARDINAL): Status;
```

Resolves `host` (either a hostname like `"example.com"` or a dotted-quad like `"127.0.0.1"`) using `getaddrinfo` and connects the socket. The underlying resolution is IPv4-only (`AF_INET`). The socket must have been created with `SOCK_STREAM`. Returns `SysError` if DNS resolution or the connect call fails; `Invalid` if `s` is `InvalidSocket`.

```modula2
st := Connect(sock, "localhost", 8080);
```

## I/O

### SendBytes

```modula2
PROCEDURE SendBytes(s: Socket;
                    VAR buf: ARRAY OF BYTE;
                    len: CARDINAL;
                    VAR sent: CARDINAL): Status;
```

Sends up to `len` bytes from `buf`. On success, `sent` contains the number of bytes actually accepted by the OS, which may be less than `len` (partial send). The caller must loop if all bytes must be delivered. `sent` is set to 0 before the call. Returns `WouldBlock` on a non-blocking socket with a full send buffer, `SysError` on other OS errors, or `Invalid` if `s` is `InvalidSocket`.

### RecvBytes

```modula2
PROCEDURE RecvBytes(s: Socket;
                    VAR buf: ARRAY OF BYTE;
                    max: CARDINAL;
                    VAR got: CARDINAL): Status;
```

Receives up to `max` bytes into `buf`. On success, `got` contains the actual byte count. Returns `Closed` when the peer has shut down the connection (recv returns 0). Returns `WouldBlock` on a non-blocking socket with no available data. `got` is set to 0 before the call.

```modula2
st := RecvBytes(sock, buf, SIZE(buf), n);
IF st = Closed THEN (* peer disconnected *) END;
```

## Convenience

### SendString

```modula2
PROCEDURE SendString(s: Socket; str: ARRAY OF CHAR): Status;
```

Sends the NUL-terminated string `str`, excluding the terminating NUL. Internally loops over `send` until all bytes have been transmitted, so partial sends are handled automatically. Returns `OK` immediately if the string is empty. Returns `SysError` if any send call fails.

```modula2
st := SendString(sock, "GET / HTTP/1.0\r\n\r\n");
```

### RecvLine

```modula2
PROCEDURE RecvLine(s: Socket; VAR line: ARRAY OF CHAR): Status;
```

Reads bytes one at a time until a LF (`0AH`) is encountered or the buffer is full. Strips a trailing CR+LF pair, producing a NUL-terminated string in `line`. If the buffer fills before a LF arrives, reading continues (consuming the remainder of the line) but excess characters are discarded. Returns `Closed` only if the peer disconnects before any data is received; if some data was already buffered, that partial line is returned with `OK`. Returns `WouldBlock` or `SysError` under the same conditions as `RecvBytes` when no data has been buffered yet.

```modula2
st := RecvLine(sock, buf);
(* buf now contains one line without CR/LF *)
```

## Non-blocking

### SetNonBlocking

```modula2
PROCEDURE SetNonBlocking(s: Socket; enable: BOOLEAN): Status;
```

Sets or clears the `O_NONBLOCK` flag on the socket using `fcntl`. Pass `TRUE` to enable non-blocking mode, `FALSE` to revert to blocking. Returns `Invalid` if `s` is `InvalidSocket`; `SysError` if the `fcntl` call fails.

```modula2
st := SetNonBlocking(sock, TRUE);
(* subsequent RecvBytes may now return WouldBlock *)
```

## Error Info

### GetLastErrno

```modula2
PROCEDURE GetLastErrno(): INTEGER;
```

Returns the raw `errno` value from the most recent failed C bridge call. Only meaningful immediately after a procedure returns `SysError`. The value is platform-dependent (e.g. `EAGAIN` is 35 on macOS, 11 on Linux).

### GetLastErrorText

```modula2
PROCEDURE GetLastErrorText(VAR out: ARRAY OF CHAR);
```

Copies the `strerror` description of the last errno into `out` as a NUL-terminated string. The buffer should be at least 128 characters to avoid truncation. Uses the XSI-compliant `strerror_r` on both Linux and macOS.

```modula2
VAR errBuf: ARRAY [0..127] OF CHAR;
...
GetLastErrorText(errBuf);
```

## Complete Example

A minimal TCP echo server that accepts one connection and echoes lines back.

```modula2
MODULE EchoServer;

FROM Sockets IMPORT
  Socket, SockAddr, Status,
  InvalidSocket, AF_INET, SOCK_STREAM, SHUT_RDWR,
  OK, Closed, SysError,
  SocketCreate, CloseSocket, Shutdown,
  Bind, Listen, Accept,
  SendString, RecvLine,
  GetLastErrorText;
FROM InOut IMPORT WriteString, WriteLn;

VAR
  server, client: Socket;
  peer: SockAddr;
  st: Status;
  line: ARRAY [0..1023] OF CHAR;
  errBuf: ARRAY [0..127] OF CHAR;

BEGIN
  st := SocketCreate(AF_INET, SOCK_STREAM, server);
  IF st # OK THEN
    WriteString("SocketCreate failed"); WriteLn; HALT
  END;

  st := Bind(server, 9000);
  IF st # OK THEN
    GetLastErrorText(errBuf);
    WriteString("Bind: "); WriteString(errBuf); WriteLn; HALT
  END;

  st := Listen(server, 8);
  WriteString("Listening on port 9000..."); WriteLn;

  st := Accept(server, client, peer);
  IF st # OK THEN
    WriteString("Accept failed"); WriteLn; HALT
  END;
  WriteString("Client connected"); WriteLn;

  LOOP
    st := RecvLine(client, line);
    IF st = Closed THEN EXIT END;
    IF st # OK THEN EXIT END;
    st := SendString(client, line);
    IF st # OK THEN EXIT END;
    st := SendString(client, "\r\n");
    IF st # OK THEN EXIT END
  END;

  st := Shutdown(client, SHUT_RDWR);
  st := CloseSocket(client);
  st := CloseSocket(server);
  WriteString("Done"); WriteLn
END EchoServer.
```
