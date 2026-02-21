# m2stream

Transport-agnostic byte stream for async networking in Modula-2+
programs. Unifies TCP sockets and TLS sessions behind a single
interface.

The library wraps m2sockets and m2tls behind an opaque `Stream`
handle, providing both synchronous (try-once) and asynchronous
(Future-returning) I/O operations. All state lives in a heap-allocated
`StreamRec` -- no hidden globals, no threads.

The error model uses `Status` return codes for every operation. The
sync API returns `WouldBlock` when a TLS session needs to retry in a
different I/O direction; the async API handles retries internally and
settles the returned Future when the operation completes or fails.

## Modules

| Module | Description |
|---|---|
| [Stream](Stream.md) | Stream lifecycle, sync/async read/write, close, state queries |

## Manifest Configuration

Projects that depend on m2stream need the library source path and
its transitive dependencies in their `m2.toml` manifest:

```toml
includes=src ../../libs/m2stream/src ../../libs/m2tls/src ../../libs/m2evloop/src ../../libs/m2futures/src ../../libs/m2sockets/src

[cc]
extra-c=../../libs/m2tls/src/tls_bridge.c ../../libs/m2evloop/src/poller_bridge.c ../../libs/m2sockets/src/sockets_bridge.c
libs=-lssl -lcrypto
```

Adjust the relative paths to match your project layout.

With m2pkg, declare the dependency directly and let the package
manager resolve transitive includes:

```toml
[deps]
m2stream=path:../../libs/m2stream
```

## See Also

- [Stream](Stream.md) -- Full API reference
- [Stream-Architecture](Stream-Architecture.md) -- Internal design and layering
- [stream_usage_example](stream_usage_example.md) -- Usage examples
- [../m2tls/TLS](../m2tls/TLS.md) -- TLS transport layer
- [../m2sockets/Sockets](../m2sockets/Sockets.md) -- Socket layer
- [../m2http/HTTPClient](../m2http/HTTPClient.md) -- HTTP client (consumer of Stream)
