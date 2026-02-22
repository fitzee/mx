# Http2ServerTestUtil

Deterministic test harness for the HTTP/2 server. Provides frame
builders, a frame reader, and test connection helpers that bypass
TLS and real sockets.

## Frame Builders (client → server)

| Procedure | Description |
|---|---|
| `BuildClientPreface` | 24-byte connection preface |
| `BuildSettings` | SETTINGS frame |
| `BuildSettingsAck` | SETTINGS ACK frame |
| `BuildHeaders` | HEADERS with HPACK-encoded entries |
| `BuildData` | DATA frame with payload |
| `BuildWindowUpdate` | WINDOW_UPDATE frame |
| `BuildPing` | PING frame |
| `BuildGoaway` | GOAWAY frame |
| `BuildRstStream` | RST_STREAM frame |
| `BuildContinuation` | Raw CONTINUATION (for violation testing) |

## Convenience Builders

```modula2
PROCEDURE BuildGET(VAR buf: Buf; VAR dt: DynTable;
                   streamId: CARDINAL; path: ARRAY OF CHAR);
PROCEDURE BuildPOST(VAR buf: Buf; VAR dt: DynTable;
                    streamId: CARDINAL; path: ARRAY OF CHAR);
```

Build complete GET/POST HEADERS frames with :method, :path,
:scheme (https), and :authority (localhost).

## Frame Reader (server → client)

```modula2
PROCEDURE ReadNextFrame(VAR v: BytesView; VAR hdr: FrameHeader;
                        VAR payload: BytesView): BOOLEAN;
```

Parse the next frame from a BytesView. Advances view past the
consumed frame.

## Test Connection Helpers

```modula2
PROCEDURE FeedAndCollect(cp: ConnPtr; VAR input: Buf; VAR output: Buf);
PROCEDURE DoTestHandshake(cp: ConnPtr; VAR output: Buf): BOOLEAN;
```

`FeedAndCollect` feeds input bytes and collects server output.
`DoTestHandshake` performs a complete H2 handshake.

## Testing Pattern

```modula2
VAR cp: ConnPtr; input, output: Buf;

ConnCreateTest(NIL, 1, cp);
Init(input, 1024);
Init(output, 4096);

(* Handshake *)
BuildClientPreface(input);
BuildSettings(input, settings);
FeedAndCollect(cp, input, output);

(* Send request *)
Clear(input);
BuildGET(input, dt, 1, "/hello");
FeedAndCollect(cp, input, output);

(* Parse response frames *)
v := AsView(output);
WHILE ReadNextFrame(v, hdr, payload) DO
  (* check hdr.ftype, payload *)
END;

ConnClose(cp);
```
