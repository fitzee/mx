# HTTPS GET Example

Walkthrough of `example_apps/https_get.mod` -- a complete HTTPS GET client demonstrating TLS verification.

## What It Does

Fetches `https://example.com/` over TLS and prints:
- HTTP status code
- Content-Type header
- Body length
- First 512 bytes of response body

TLS peer verification is ON by default. The connection uses TLS 1.2+ with the system CA root store.

## Build

```bash
m2c --m2plus example_apps/https_get.mod \
  -I libs/m2http/src \
  -I libs/m2tls/src \
  -I libs/m2evloop/src \
  -I libs/m2futures/src \
  -I libs/m2sockets/src \
  libs/m2http/src/dns_bridge.c \
  libs/m2tls/src/tls_bridge.c \
  libs/m2evloop/src/poller_bridge.c \
  libs/m2sockets/src/sockets_bridge.c \
  -lssl -lcrypto
```

## Expected Output

```
HTTP 200
Content-Type: text/html; charset=UTF-8
Body: 1256 bytes

--- Body (first 512 bytes) ---
<!doctype html>
<html>
<head>
    <title>Example Domain</title>
...
```

## Code Walkthrough

### 1. Parse URL

```modula2
url := "https://example.com/";
ust := Parse(url, uri);
```

`URI.Parse` decomposes the URL into `scheme="https"`, `host="example.com"`, `port=443`, `path="/"`.

### 2. Create Event Loop

```modula2
est := Create(loop);
sched := GetScheduler(loop);
```

Creates the event loop with its internal Poller, Timers, and Scheduler.

### 3. Issue HTTPS GET Request

```modula2
hst := Get(loop, sched, uri, future);
```

Because the URI scheme is `https`, `HTTPClient.Get` internally:

1. Resolves `example.com` to an IPv4 address (blocking DNS)
2. Creates a TLS context with `TLS.ContextCreate`
3. Sets `VerifyPeer` mode (default) and loads system roots
4. Creates a non-blocking TCP socket and initiates connect
5. Creates a TLS session with `TLS.SessionCreate`
6. Sets SNI to `example.com` with `TLS.SetSNI`
7. Registers the socket with the event loop
8. Returns `OK` with a Future that will settle when the response arrives

The TLS handshake happens automatically as part of the connection state machine: after TCP connect completes, HTTPClient enters `StHandshaking` and drives `TLS.Handshake` calls until the handshake succeeds.

### 4. Event Loop Flow

```
TCP connect (non-blocking)
    ↓ write-ready
getsockopt(SO_ERROR) = 0
    ↓
TLS.Handshake → WantWrite → wait
    ↓ write-ready
TLS.Handshake → WantRead → wait
    ↓ read-ready
TLS.Handshake → OK
    ↓
TLS.Write (HTTP request) → send encrypted
    ↓
TLS.Read (HTTP response) → recv + decrypt
    ↓
Parse status + headers + body
    ↓
Resolve(promise, response)
    ↓
OnCheck timer fires → future is settled
```

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

When the future settles, extract the `ResponsePtr`, read status/headers/body, then free the response and stop the event loop.

## TLS Is Transparent

From the application's perspective, the only difference between HTTP and HTTPS is the URL scheme:

| HTTP                                    | HTTPS                                     |
|-----------------------------------------|-------------------------------------------|
| `url := "http://httpbin.org/get";`      | `url := "https://example.com/";`          |
| Port 80 (default)                       | Port 443 (default)                        |
| `HTTPClient.Get(loop, sched, uri, fut)` | `HTTPClient.Get(loop, sched, uri, fut)`   |
| Same API, no TLS parameters needed      | TLS setup is automatic inside HTTPClient  |

## Key Patterns

- **Verification by default**: No code needed to enable TLS verification. HTTPClient creates the TLS context with `VerifyPeer` and loads system roots automatically.
- **SNI automatic**: HTTPClient extracts the hostname from the URI and sets SNI on the TLS session.
- **Cleanup automatic**: HTTPClient performs TLS shutdown and destroys the TLS session/context in its cleanup path. The application only calls `FreeResponse` for the response data.
- **Error propagation**: If TLS handshake fails (e.g., `VerifyFailed`), the Future is rejected with error code 6 (TLS failure).

## See Also

- [../m2tls/TLS](TLS.md) -- TLS API reference
- [../m2tls/TLS-Architecture](TLS-Architecture.md) -- TLS internal design
- [../m2http/HTTPClient](../m2http/HTTPClient.md) -- HTTP client API
- [../m2http/http_get_example](../m2http/http_get_example.md) -- Plain HTTP example
