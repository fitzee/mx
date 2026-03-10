# Net Architecture

Overview of the mx networking stack, from OS sockets up to the HTTP client.

## Layer Diagram

```
┌──────────────────────────────────────────────────────────┐
│  Application Code                                        │
│  (http_get.mod, https_get.mod, custom clients, etc.)     │
├──────────────────────────────────────────────────────────┤
│  HTTPClient                   (m2http)                   │
│  State-machine HTTP/1.1 client: connect, send, recv,     │
│  parse status/headers/body, resolve Future               │
│  HTTPS: TLS handshake + encrypted I/O via m2tls          │
├────────────────┬────────────┬────────────────────────────┤
│  URI           │  DNS       │  Buffers                   │
│  URL parsing   │  Hostname  │  Binary-safe I/O buffers   │
│  & path build  │  resolve   │  with zero-copy access     │
├────────────────┴────────────┴────────────────────────────┤
│  TLS                          (m2tls)                    │
│  OpenSSL wrapper: context, session, handshake, R/W       │
├──────────────────────────────────────────────────────────┤
│  Promise / Future / Scheduler          (m2futures)       │
│  Composable async values, microtask queue                │
├──────────────────────────────────────────────────────────┤
│  EventLoop / Timers / Poller           (m2evloop)        │
│  fd readiness polling, timer heap, scheduler pump        │
├──────────────────────────────────────────────────────────┤
│  Sockets                               (m2sockets)      │
│  TCP/UDP socket creation, bind, listen, connect, I/O     │
├──────────────────────────────────────────────────────────┤
│  C Bridges (FFI)                                         │
│  dns_bridge.c  poller_bridge.c  sockets_bridge.c         │
│  tls_bridge.c                                            │
├──────────────────────────────────────────────────────────┤
│  OS Kernel + OpenSSL/LibreSSL                            │
│  BSD sockets, kqueue/epoll/poll, getaddrinfo, libssl     │
└──────────────────────────────────────────────────────────┘
```

## Library Dependencies

```
m2http
├── m2tls       (TLS context, session, handshake, R/W)
│   ├── m2evloop    (EventLoop, Poller, Timers)
│   │   └── m2futures  (Scheduler, Promise)
│   └── m2futures   (Scheduler, Promise)
├── m2evloop    (EventLoop, Poller, Timers)
│   └── m2futures  (Scheduler, Promise)
├── m2futures   (Promise, Scheduler)
└── m2sockets   (Sockets, SocketsBridge)
```

## Data Flow: HTTP GET Request

```
1. Application calls HTTPClient.Get(loop, sched, uri, future)
   │
2. DNS.ResolveA(host, port)
   │  └─ dns_bridge.c → getaddrinfo → AddrRec
   │
3. DoRequest allocates ConnRec, Response, recvBuf
   │  Builds "GET /path HTTP/1.1\r\nHost: ...\r\n\r\n"
   │  Creates non-blocking TCP socket
   │  Initiates connect (may return EINPROGRESS)
   │  Registers fd with EventLoop.WatchFd
   │
4. Application calls EventLoop.Run(loop)
   │
5. Event loop dispatches OnSocketEvent for each fd readiness:
   │
   ├── StConnecting: check getsockopt(SO_ERROR), transition to StSending
   ├── StSending: m2_send() request bytes, transition to StRecvStatus
   ├── StRecvStatus: m2_recv() → ParseStatusLine, transition to StRecvHeaders
   ├── StRecvHeaders: ParseHeaders (Content-Length, Transfer-Encoding)
   │   └── transition to StRecvBody (or SucceedConn for HEAD)
   └── StRecvBody: recv body (Content-Length or chunked)
       └── SucceedConn: transfer body, Resolve(promise, response)
           └── Application's Future is settled
```

## Connection Lifecycle

```
         ┌─────────────┐
         │ DoRequest    │ DNS resolve, allocate, connect
         └──────┬──────┘
                │
         ┌──────▼──────┐
         │ Connecting   │ wait for write-ready
         └──────┬──────┘
                │ getsockopt OK
         ┌──────▼──────┐
         │ Sending      │ send request bytes
         └──────┬──────┘
                │ all sent
         ┌──────▼──────┐
         │ RecvStatus   │ parse "HTTP/1.1 200 OK\r\n"
         └──────┬──────┘
                │
         ┌──────▼──────┐
         │ RecvHeaders  │ parse headers until blank line
         └──────┬──────┘
                │
         ┌──────▼──────┐
         │ RecvBody     │ Content-Length or chunked
         └──────┬──────┘
                │ complete
         ┌──────▼──────┐
         │ SucceedConn  │ Resolve(promise, response)
         └─────────────┘

   At any point, errors lead to FailConn → Reject(promise, error)
```

## Resource Budget

| Resource          | Per Request | Source          |
|-------------------|-------------|-----------------|
| ConnRec           | ~4 KB       | ALLOCATE        |
| Response          | ~36 KB      | ALLOCATE        |
| recv Buffer       | ~65 KB      | Buffers.Create  |
| body Buffer       | ~65 KB      | Buffers.Create  |
| Socket fd         | 1           | SocketCreate    |
| EventLoop watcher | 1 of 64     | WatchFd         |
| Promise slot      | 1 of 256    | PromiseCreate   |
| **Total**         | **~170 KB** |                 |

## Chunked Transfer Encoding

When the server sends `Transfer-Encoding: chunked`, the body is delivered as a series of chunks:

```
<chunk-size-hex>\r\n
<chunk-data>\r\n
...
0\r\n
\r\n
```

The `ProcessChunked` state machine cycles through:

1. **ChSize**: Read hex size line → if 0, done
2. **ChData**: Copy `chunkRem` bytes from recv to body buffer
3. **ChTrailer**: Consume the `\r\n` after chunk data
4. Back to ChSize

## Platform Support

| Platform | Sockets | DNS         | I/O Polling | Build                          |
|----------|---------|-------------|-------------|--------------------------------|
| macOS    | BSD     | getaddrinfo | kqueue      | cc + dns_bridge.c              |
| Linux    | BSD     | getaddrinfo | epoll       | cc + dns_bridge.c              |
| Others   | BSD     | getaddrinfo | poll        | cc + dns_bridge.c              |

## Limitations

- **HTTPS**: Supported via m2tls (OpenSSL/LibreSSL). Peer verification ON by default.
- **No connection pooling**: Each request opens and closes a connection.
- **Blocking DNS**: DNS resolution blocks the event loop thread.
- **64 KB body limit**: Responses larger than `Buffers.MaxCap` are truncated.
- **Single-threaded**: All I/O on the event loop thread.

## Future Extension Points

- ~~**TLS**~~: Done. m2tls wraps OpenSSL/LibreSSL for HTTPS.
- **Connection pool**: Reuse TCP connections for same host:port.
- **Async DNS**: Background thread or c-ares integration.
- **Streaming**: Callback-based body delivery for large responses.
- **WebSocket**: Upgrade-based WebSocket client on the same event loop.
- **HTTP server**: Accept + route + respond using the same architecture.

## See Also

- [Buffers](Buffers.md) — I/O buffer module
- [URI](URI.md) — URI parser module
- [DNS](DNS.md) — DNS resolver module
- [HTTPClient](HTTPClient.md) — HTTP client module
- [../m2evloop/Async-Architecture](../m2evloop/Async-Architecture.md) — Event loop architecture
- [../m2futures/Promise](../m2futures/Promise.md) — Promise/Future types
- [../m2tls/TLS](../m2tls/TLS.md) — TLS transport layer
- [../m2tls/TLS-Architecture](../m2tls/TLS-Architecture.md) — TLS internal design
