# TLS Integration

## Server-Side TLS Setup

The HTTP/2 server uses m2TLS server-side extensions:

1. **Server context**: `TLS.ContextCreateServer()` creates a context
   with `TLS_server_method()` and `SSL_VERIFY_NONE` (servers don't
   verify clients by default).

2. **Certificate loading**: `TLS.SetServerCert(ctx, certPath, keyPath)`
   loads PEM certificate chain and private key.

3. **ALPN negotiation**: `TLS.SetALPNServer(ctx, protos, protosLen)`
   installs a callback that matches client-offered protocols against
   the server's list. The server advertises `h2` only.

## ALPN Wire Format

```
\x02h2    (* length-prefixed: 2 bytes "h2" *)
```

The server rejects connections that don't negotiate `h2`.

## Per-Connection TLS

Each accepted connection gets its own TLS session via
`TLS.SessionCreateServer()`. This uses `SSL_set_accept_state()`
instead of `SSL_set_connect_state()`.

The handshake is driven by the event loop:
- Watcher on client fd for read events
- `TLS.Handshake()` returns WantRead/WantWrite during negotiation
- On completion, `TLS.GetALPN()` verifies "h2" was negotiated

## Certificate Requirements

- PEM format (both cert and key)
- Key must match certificate
- For development: generate self-signed cert with openssl

```bash
openssl req -x509 -newkey rsa:2048 -keyout server.key \
  -out server.crt -days 365 -nodes -subj "/CN=localhost"
```
