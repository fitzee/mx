# HTTP GET Example

Walkthrough of `examples/networking/http_get.mod` — a complete HTTP GET client.

## What It Does

Fetches `http://httpbin.org/get` and prints:
- HTTP status code
- Content-Type header
- Body length
- First 512 bytes of response body

## Build

```bash
mx --m2plus examples/networking/http_get.mod \
  -I libs/m2http/src \
  -I libs/m2evloop/src \
  -I libs/m2futures/src \
  -I libs/m2sockets/src \
  libs/m2http/src/dns_bridge.c \
  libs/m2evloop/src/poller_bridge.c \
  libs/m2sockets/src/sockets_bridge.c
```

## Expected Output

```
HTTP 200
Content-Type: application/json
Body: 287 bytes

--- Body (first 512 bytes) ---
{
  "args": {},
  "headers": {
    "Connection": "close",
    "Host": "httpbin.org",
    "User-Agent": "m2http/0.1"
  },
  "origin": "...",
  "url": "http://httpbin.org/get"
}
```

## Code Walkthrough

### 1. Parse URL

```modula2
url := "http://httpbin.org/get";
ust := Parse(url, uri);
```

`URI.Parse` decomposes the URL into `scheme="http"`, `host="httpbin.org"`, `port=80`, `path="/get"`.

### 2. Create Event Loop

```modula2
est := Create(loop);
sched := GetScheduler(loop);
```

Creates the event loop with its internal Poller, Timers, and Scheduler.

### 3. Issue GET Request

```modula2
hst := Get(loop, sched, uri, future);
```

This:
1. Resolves `httpbin.org` to an IPv4 address (blocking DNS)
2. Allocates a connection context and response record
3. Builds the HTTP request: `GET /get HTTP/1.1\r\nHost: httpbin.org\r\n...`
4. Creates a non-blocking TCP socket
5. Initiates the connect (likely returns EINPROGRESS)
6. Registers the socket with the event loop
7. Returns `OK` with `future` that will settle when the response arrives

### 4. Poll for Completion

```modula2
est := EventLoop.SetInterval(loop, 50, OnCheck, NIL, tid);
Run(loop);
```

A 50ms interval timer checks if the future has settled. The event loop concurrently drives the HTTP connection forward via socket events.

### 5. Handle Response

```modula2
PROCEDURE OnCheck(user: ADDRESS);
VAR settled: BOOLEAN; res: Result; resp: ResponsePtr;
BEGIN
  pst := GetResultIfSettled(future, settled, res);
  IF NOT settled THEN RETURN END;
  resp := res.v.ptr;
  (* use resp^.statusCode, resp^.headers, resp^.body *)
  FreeResponse(resp);
  EventLoop.Stop(loop)
END OnCheck;
```

When settled, extract the `ResponsePtr`, read status/headers/body, then free the response and stop the event loop.

## Key Patterns

- **Futures for results**: The HTTP response is delivered through a Future, not a blocking call.
- **Event loop drives I/O**: `Run(loop)` pumps the event loop, which handles socket readiness and timer ticks.
- **Timer-based polling**: A simple interval timer checks the Future. In production code, use `Promise.Map` or `Promise.OnSettle` for callback-based notification.
- **Resource cleanup**: `FreeResponse` frees the response record and its body buffer. `Destroy(loop)` frees the event loop.

## See Also

- [HTTPClient](HTTPClient.md) — Full API reference
- [URI](URI.md) — URI parsing
- [../m2evloop/EventLoop](../m2evloop/EventLoop.md) — Event loop API
