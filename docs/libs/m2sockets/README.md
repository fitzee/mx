# m2sockets

Minimal POSIX/BSD sockets wrapper for TCP and UDP networking in
Modula-2+ programs. Targets Linux and macOS.

The library is structured as a thin C bridge layer (13 functions
wrapping POSIX syscalls) with all higher-level logic written in
Modula-2. It provides both blocking and non-blocking I/O through
a single module.

The error model uses `Status` return codes for every operation.
When a call returns `SysError`, use `GetLastErrno()` or
`GetLastErrorText` to retrieve the underlying OS error.

## Modules

| Module | Description |
|---|---|
| [Sockets](Sockets.md) | TCP/UDP socket lifecycle, I/O, server/client operations, non-blocking mode |

SocketsBridge is an internal module that provides the raw C FFI
bindings to POSIX socket syscalls. It is not intended for direct use.

## Manifest Configuration

Projects that depend on m2sockets need the library source path and
C bridge file in their `m2.toml` manifest:

```toml
includes=src ../../libs/m2sockets/src

[cc]
extra-c=../../libs/m2sockets/src/sockets_bridge.c
```

Adjust the relative path to match your project layout.
