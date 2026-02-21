# DNS

Minimal async DNS resolver returning Futures. Wraps the system `getaddrinfo` via a C bridge.

## Overview

`DNS` provides a Futures-compatible API for hostname resolution. The current implementation calls `getaddrinfo` synchronously and returns an already-settled Future. This preserves the async API contract so callers need no changes when a truly asynchronous backend is added later.

## Design Goals

- **Futures-first API**: Callers always work with Futures, regardless of whether resolution is sync or async.
- **Minimal surface**: Single procedure (`ResolveA`) for IPv4 A-record lookup.
- **Clean separation**: C bridge (`dns_bridge.c`) isolates platform DNS calls; M2 module handles Promise/Future lifecycle.

## Architecture

```
┌─────────────────────────────────────┐
│  HTTPClient.DoRequest               │
│  calls DNS.ResolveA(host, port)     │
├─────────────────────────────────────┤
│  DNS module (M2)                    │
│  1. Create Promise/Future pair      │
│  2. Allocate AddrRec on heap        │
│  3. Call m2_dns_resolve_a (blocking) │
│  4. Resolve or reject promise       │
├─────────────────────────────────────┤
│  dns_bridge.c (C FFI)              │
│  getaddrinfo → first AF_INET result │
│  copies IPv4 address + port         │
├─────────────────────────────────────┤
│  OS resolver (libc)                 │
└─────────────────────────────────────┘
```

## Internal Data Structures

```modula2
TYPE
  AddrRec = RECORD
    addrV4 : ARRAY [0..3] OF BYTE;   (* IPv4 address, network order *)
    port   : INTEGER;                 (* port number *)
  END;
```

`AddrRec` is heap-allocated via `ALLOCATE` and returned through `Value.ptr` in the resolved Future. The caller (typically `HTTPClient`) must `DEALLOCATE` it when done.

## Memory Model

| Phase       | Allocation          | Owner              |
|-------------|---------------------|--------------------|
| ResolveA    | 1 AddrRec (heap)    | DNS module         |
| On resolve  | transferred via ptr | Future consumer    |
| On reject   | DEALLOCATE'd        | DNS module         |

## Error Model

| Status         | Meaning                                  |
|----------------|------------------------------------------|
| `OK`           | Resolution succeeded. Future is settled. |
| `Invalid`      | NIL scheduler.                           |
| `ResolveFailed`| getaddrinfo returned no results.         |
| `OutOfMemory`  | Heap allocation or Promise pool failed.  |

On `ResolveFailed` or `OutOfMemory`, the Future is still created and rejected (if a Promise could be allocated), so callers always get a valid Future.

## Performance Characteristics

- **ResolveA**: Blocking. Duration depends on system DNS resolver (typically 1-100ms for cached, 100-5000ms for uncached lookups).
- No caching. Each call invokes `getaddrinfo`.
- No concurrent resolution. The calling thread blocks during lookup.

## Limitations

- **Blocking**: Current implementation blocks during DNS resolution. The event loop is stalled.
- **IPv4 only**: Returns the first A record. No AAAA (IPv6) support.
- **No caching**: Every call triggers a fresh `getaddrinfo`.
- **No timeout**: Resolution timeout is controlled by the OS resolver.
- **No round-robin**: Always returns the first address.

## Future Extension Points

- Thread-pool-based async resolution (resolve in background, settle Future on completion).
- IPv6 (AAAA) support via `ResolveAAAA` procedure.
- DNS response caching with TTL expiry.
- Custom DNS server support (bypass system resolver).
- Round-robin or random selection from multiple A records.

## API Reference

### Types

**`AddrRec`** — Resolved address:

```modula2
TYPE AddrRec = RECORD
  addrV4 : ARRAY [0..3] OF BYTE;
  port   : INTEGER;
END;
```

**`AddrPtr`** — `POINTER TO AddrRec`.

**`Status`** — `(OK, Invalid, ResolveFailed, OutOfMemory)`.

### Procedures

```modula2
PROCEDURE ResolveA(lp: Loop; sched: Scheduler;
                   VAR host: ARRAY OF CHAR;
                   port: INTEGER;
                   VAR outFuture: Future): Status;
```

Resolve `host` to an IPv4 address. On success, `outFuture` resolves with `Value.ptr` pointing to a heap-allocated `AddrRec`. The caller must `DEALLOCATE(ptr, TSIZE(AddrRec))` when done.

## C Bridge

The `dns_bridge.c` file provides:

```c
int m2_dns_resolve_a(const char *host,
                     unsigned char *out_addr4,
                     int *out_port, int port);
```

Returns 0 on success, -1 on failure. Also provides `m2_connect_ipv4` and `m2_getsockopt_error` used by the HTTP client.

## See Also

- [HTTPClient](HTTPClient.md) — Primary consumer of DNS resolution
- [Net-Architecture](Net-Architecture.md) — Overall networking stack design
- [../m2evloop/EventLoop](../m2evloop/EventLoop.md) — Event loop (Loop parameter)
- [../m2futures/Promise](../m2futures/Promise.md) — Future/Promise types
