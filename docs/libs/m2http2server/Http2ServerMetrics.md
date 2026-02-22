# Http2ServerMetrics

Server-wide observability counters. All fields are simple CARDINALs.

## Counters

| Counter | Procedure | Description |
|---|---|---|
| connsAccepted | `IncConnsAccepted` | Total connections accepted |
| connsActive | `IncConnsActive` / `DecConnsActive` | Currently active |
| connsClosed | `IncConnsClosed` | Total connections closed |
| tlsHandshakeFail | `IncTLSFail` | TLS handshake failures |
| alpnReject | `IncALPNReject` | ALPN negotiation failures |
| streamsOpened | `IncStreamsOpened` | Total streams opened |
| reqTotal | `IncReqTotal` | Total requests dispatched |
| resp2xx | `IncResp(code)` | 2xx responses |
| resp4xx | `IncResp(code)` | 4xx responses |
| resp5xx | `IncResp(code)` | 5xx responses |
| protoErrors | `IncProtoErrors` | Protocol errors (GOAWAY) |
| bytesIn | `AddBytesIn(n)` | Total bytes received |
| bytesOut | `AddBytesOut(n)` | Total bytes sent |

## MetricsLog

```modula2
PROCEDURE MetricsLog(VAR m: Metrics; VAR lg: Logger);
```

Logs all 13 counters as structured key-value fields at INFO level.
