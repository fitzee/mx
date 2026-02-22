# Http2ServerLog

Structured logging adapter wrapping m2Log for the HTTP/2 server.

## Procedures

### LogInit
```modula2
PROCEDURE LogInit(VAR lg: Logger);
```
Create a logger with category "h2server" and console sink.

### LogRequest
```modula2
PROCEDURE LogRequest(VAR lg: Logger; VAR req: Request;
                     VAR resp: Response; durationTicks: INTEGER);
```
Log request completion with fields: conn, stream, method, path,
status, dur.

### LogProtocol
```modula2
PROCEDURE LogProtocol(VAR lg: Logger; connId: CARDINAL;
                      event, detail: ARRAY OF CHAR);
```
Log protocol events (SETTINGS, GOAWAY, etc.).

### LogConn
```modula2
PROCEDURE LogConn(VAR lg: Logger; connId: CARDINAL;
                  event: ARRAY OF CHAR);
```
Log connection events (accepted, closed, error).
