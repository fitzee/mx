# Deployment Notes

## TLS Certificate Setup

Generate a self-signed certificate for development:

```bash
openssl req -x509 -newkey rsa:2048 -keyout server.key \
  -out server.crt -days 365 -nodes -subj "/CN=localhost"
```

For production, use certificates from a CA (Let's Encrypt, etc.).

## Port Binding

Default port is 8443. Ports below 1024 require root privileges.
Use a reverse proxy (nginx, haproxy) for port 443.

## Configuration Tuning

### MaxConns (default: 16)

Each connection uses ~1.1 MB for HPACK dynamic tables plus 32 KB
for the arena. 16 connections ≈ 18 MB base memory.

Increase by modifying `MaxConns` in Http2ServerTypes.def and
recompiling.

### MaxStreamSlots (default: 32)

Maximum concurrent streams per connection. The server advertises
this in SETTINGS. Higher values allow more multiplexing but
increase per-connection memory.

### Timeouts

| Option | Default | Purpose |
|---|---|---|
| idleTimeoutMs | 30000 | Close idle connections |
| hsTimeoutMs | 5000 | TLS handshake deadline |
| drainTimeoutMs | 10000 | GOAWAY drain period |

## Monitoring

Use `Http2ServerMetrics` to track:
- Connection rates (accepted, active, closed)
- Error rates (TLS failures, ALPN rejects, protocol errors)
- Request throughput (total, by status class)
- Bandwidth (bytes in/out)

Call `MetricsLog` periodically to emit counters.

## Limitations

- Single-threaded (event-loop model)
- No CONTINUATION frames
- No server push
- No HPACK Huffman encoding
- Exact-match routing only
- 16 max connections, 32 max streams per connection
