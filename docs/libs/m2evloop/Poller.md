# Poller

Cross-platform file descriptor readiness poller. Wraps kqueue (macOS/BSD), epoll (Linux), or poll (POSIX fallback) behind a uniform Modula-2 interface.

## Backend Selection

The C bridge (`poller_bridge.c`) selects the backend at compile time:

| Platform          | Backend  | Complexity  |
|-------------------|----------|-------------|
| macOS, FreeBSD    | kqueue   | O(1) per event |
| Linux             | epoll    | O(1) per event |
| Other POSIX       | poll     | O(n) per call  |

All backends present the same interface to Modula-2 code.

## Constants

```modula2
CONST
  EvRead  = 1;    (* fd is readable *)
  EvWrite = 2;    (* fd is writable *)
  EvError = 4;    (* error condition *)
  EvHup   = 8;    (* hangup / peer closed *)
  MaxEvents = 64; (* max events per Wait call *)
```

## Types

**`Poller`** -- Handle to a poller instance:

```modula2
TYPE Poller = INTEGER;
```

**`PollEvent`** -- Returned by `Wait`:

```modula2
TYPE PollEvent = RECORD
  fd     : INTEGER;     (* which file descriptor *)
  events : INTEGER;     (* bitmask of Ev* constants *)
END;
```

**`EventBuf`** -- Buffer for `Wait` results:

```modula2
TYPE EventBuf = ARRAY [0..MaxEvents-1] OF PollEvent;
```

**`Status`** -- Operation result:

| Value      | Meaning                              |
|------------|--------------------------------------|
| `OK`       | Operation succeeded.                 |
| `SysError` | OS-level error.                      |
| `Invalid`  | Bad argument (invalid handle).       |

## Procedures

### Create

```modula2
PROCEDURE Create(VAR out: Poller): Status;
```

Create a new poller instance. Up to 16 pollers may exist simultaneously.

### Destroy

```modula2
PROCEDURE Destroy(VAR p: Poller): Status;
```

Destroy a poller and release OS resources.

### Add

```modula2
PROCEDURE Add(p: Poller; fd, events: INTEGER): Status;
```

Register a file descriptor for the given events (`EvRead`, `EvWrite`, or both OR'd together).

### Modify

```modula2
PROCEDURE Modify(p: Poller; fd, events: INTEGER): Status;
```

Change the interest set for an already-registered fd.

### Remove

```modula2
PROCEDURE Remove(p: Poller; fd: INTEGER): Status;
```

Unregister a file descriptor from the poller.

### Wait

```modula2
PROCEDURE Wait(p: Poller; timeoutMs: INTEGER;
               VAR buf: EventBuf;
               VAR count: INTEGER): Status;
```

Wait for events. `timeoutMs = -1` blocks indefinitely, `0` returns immediately (non-blocking poll). On return, `buf[0..count-1]` contains the ready events.

### NowMs

```modula2
PROCEDURE NowMs(): INTEGER;
```

Return the current monotonic time in milliseconds. The value is a 32-bit integer that wraps at approximately 24.8 days. Use signed-difference comparison (`a - b < 0`) for correct wrap-around handling.

## Notes

- A poller can track up to 64 file descriptors when used via EventLoop (limited by `EventLoop.MaxWatchers`). The underlying C bridge has no fd limit beyond OS constraints.
- `Wait` handles `EINTR` internally and returns 0 events rather than an error.
- kqueue uses `EV_CLEAR` (edge-triggered). Applications should drain all available data when readiness is reported.

## Example

```modula2
FROM Poller IMPORT Poller, PollEvent, EventBuf, EvRead,
                   Create, Destroy, Add, Wait, NowMs;
FROM Poller IMPORT Status, OK;

VAR
  p: Poller;
  buf: EventBuf;
  count, i: INTEGER;
  st: Status;

st := Create(p);
st := Add(p, myFd, EvRead);
st := Wait(p, 1000, buf, count);  (* wait up to 1 second *)
FOR i := 0 TO count - 1 DO
  (* buf[i].fd is ready for buf[i].events *)
END;
st := Destroy(p);
```
