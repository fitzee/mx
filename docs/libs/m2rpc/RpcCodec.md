# RpcCodec

RPC message encoding and decoding using the m2bytes Codec module. Defines the m2rpc wire protocol format (version 1) and provides safe, bounds-checked serialization for Request, Response, and Error messages.

## Design

The m2rpc wire protocol uses a compact binary format with a common header followed by message-type-specific fields. All messages fit within a single RpcFrame payload.

**Common header** (6 bytes):
```
u8   version      (must be 1)
u8   msg_type     (0=Request, 1=Response, 2=Error)
u32  request_id   (big-endian, correlates request to response)
```

**Request message** (msg_type=0):
```
u16  method_len   (big-endian)
[method_len bytes]   method name (UTF-8)
u32  body_len     (big-endian)
[body_len bytes]     request body
```

**Response message** (msg_type=1):
```
u32  body_len     (big-endian)
[body_len bytes]     response body
```

**Error message** (msg_type=2):
```
u16  err_code     (big-endian, see RpcErrors module)
u16  err_msg_len  (big-endian)
[err_msg_len bytes]  error message (UTF-8)
u32  body_len     (big-endian)
[body_len bytes]     optional error body
```

All multi-byte integers use **big-endian** byte order for network portability. All decode procedures strictly validate bounds and return `ok := FALSE` on malformed input without advancing the internal cursor.

## Constants

| Name | Value | Purpose |
|------|-------|---------|
| `Version` | 1 | Wire protocol version |
| `MsgRequest` | 0 | Message type code for requests |
| `MsgResponse` | 1 | Message type code for responses |
| `MsgError` | 2 | Message type code for errors |

## Encoding Procedures

### EncodeRequest

```modula2
PROCEDURE EncodeRequest(VAR buf: Buf;
                        requestId: CARDINAL;
                        method: ARRAY OF CHAR;
                        methodLen: CARDINAL;
                        body: BytesView);
```

Encode an RPC request into `buf`. The buffer is cleared before encoding (any prior contents are discarded).

- `requestId`: correlation ID (client-generated, echoed in the response)
- `method`: method name as an open CHAR array
- `methodLen`: number of characters in the method name (not including null terminator)
- `body`: request payload (may be empty with `len=0`)

```modula2
Init(reqBuf, 256);
EncodeRequest(reqBuf, 42, "Sum", 3, argsView);
(* reqBuf now contains: [01 00 00 00 00 2A 00 03 'S' 'u' 'm' <body>] *)
```

### EncodeResponse

```modula2
PROCEDURE EncodeResponse(VAR buf: Buf;
                         requestId: CARDINAL;
                         body: BytesView);
```

Encode an RPC response. The buffer is cleared before encoding.

- `requestId`: must match the request's ID
- `body`: response payload (may be empty)

### EncodeError

```modula2
PROCEDURE EncodeError(VAR buf: Buf;
                      requestId: CARDINAL;
                      errCode: CARDINAL;
                      errMsg: ARRAY OF CHAR;
                      errMsgLen: CARDINAL;
                      body: BytesView);
```

Encode an RPC error message. The buffer is cleared before encoding.

- `requestId`: must match the request's ID
- `errCode`: numeric error code (see RpcErrors module)
- `errMsg`: human-readable error message
- `errMsgLen`: number of characters in the error message
- `body`: optional error body (additional diagnostic data)

```modula2
EncodeError(errBuf, 42, Timeout, "request timed out", 17, emptyView);
```

## Decoding Procedures

All decode procedures validate bounds strictly. If the payload is too short or the encoding is malformed, `ok` is set to `FALSE` and no output parameters are modified.

### DecodeHeader

```modula2
PROCEDURE DecodeHeader(payload: BytesView;
                       VAR version: CARDINAL;
                       VAR msgType: CARDINAL;
                       VAR requestId: CARDINAL;
                       VAR ok: BOOLEAN);
```

Decode the common 6-byte header. On success, sets `version`, `msgType`, `requestId`, and `ok := TRUE`. The procedure advances an internal cursor past the header (so subsequent DecodeRequest/Response/Error calls see the message body).

If the payload is shorter than 6 bytes, sets `ok := FALSE`.

### DecodeRequest

```modula2
PROCEDURE DecodeRequest(payload: BytesView;
                        VAR requestId: CARDINAL;
                        VAR method: BytesView;
                        VAR body: BytesView;
                        VAR ok: BOOLEAN);
```

Decode a complete request message. The `payload` parameter must be the **full frame payload** including the header.

On success:
- `requestId`: correlation ID from the header
- `method`: zero-copy view into the method name bytes (UTF-8)
- `body`: zero-copy view into the request body
- `ok := TRUE`

If the message is truncated or the `msg_type` is not `MsgRequest`, sets `ok := FALSE`.

```modula2
DecodeRequest(framePayload, reqId, methodView, bodyView, ok);
IF ok THEN
  (* methodView.base points into framePayload *)
END;
```

### DecodeResponse

```modula2
PROCEDURE DecodeResponse(payload: BytesView;
                         VAR requestId: CARDINAL;
                         VAR body: BytesView;
                         VAR ok: BOOLEAN);
```

Decode a response message. Same semantics as DecodeRequest, but extracts only the `body` (responses have no method field).

### DecodeError

```modula2
PROCEDURE DecodeError(payload: BytesView;
                      VAR requestId: CARDINAL;
                      VAR errCode: CARDINAL;
                      VAR errMsg: BytesView;
                      VAR body: BytesView;
                      VAR ok: BOOLEAN);
```

Decode an error message.

On success:
- `requestId`: correlation ID
- `errCode`: numeric error code
- `errMsg`: zero-copy view into the error message bytes (UTF-8)
- `body`: zero-copy view into the optional error body
- `ok := TRUE`

## Example

```modula2
MODULE CodecDemo;

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, AsView, ViewGetByte;
FROM RpcCodec IMPORT EncodeRequest, DecodeRequest,
                     Version, MsgRequest;

VAR
  reqBuf, bodyBuf: Buf;
  bodyView, payload, decodedMethod, decodedBody: BytesView;
  reqId: CARDINAL;
  ok: BOOLEAN;

BEGIN
  (* Encode a request *)
  Init(bodyBuf, 16);
  (* ... fill bodyBuf with request data ... *)
  bodyView := AsView(bodyBuf);

  Init(reqBuf, 256);
  EncodeRequest(reqBuf, 123, "GetUser", 7, bodyView);

  WriteString("encoded length: ");
  WriteCard(reqBuf.len, 0); WriteLn;

  (* Decode it back *)
  payload := AsView(reqBuf);
  DecodeRequest(payload, reqId, decodedMethod, decodedBody, ok);

  IF ok THEN
    WriteString("requestId: ");
    WriteCard(reqId, 0); WriteLn;  (* 123 *)

    WriteString("method length: ");
    WriteCard(decodedMethod.len, 0); WriteLn;  (* 7 *)
  END;

  Free(reqBuf);
  Free(bodyBuf)
END CodecDemo.
```
