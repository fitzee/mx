IMPLEMENTATION MODULE RpcCodec;

FROM ByteBuf IMPORT Buf, BytesView, Clear, AppendByte, AppendView;
FROM Codec IMPORT Reader, Writer, InitReader, InitWriter,
                   ReadU8, ReadU16BE, ReadU32BE, ReadSlice, Remaining,
                   WriteU8, WriteU16BE, WriteU32BE, WriteChars;

(* ── Encoding ─────────────────────────────────────────── *)

PROCEDURE WriteHeader(VAR w: Writer; msgType, requestId: CARDINAL);
BEGIN
  WriteU8(w, Version);
  WriteU8(w, msgType);
  WriteU32BE(w, requestId)
END WriteHeader;

PROCEDURE EncodeRequest(VAR buf: Buf;
                        requestId: CARDINAL;
                        method: ARRAY OF CHAR;
                        methodLen: CARDINAL;
                        body: BytesView);
VAR w: Writer; ml: CARDINAL;
BEGIN
  Clear(buf);
  InitWriter(w, buf);
  WriteHeader(w, MsgRequest, requestId);
  ml := methodLen;
  IF ml > HIGH(method) + 1 THEN ml := HIGH(method) + 1 END;
  WriteU16BE(w, ml);
  WriteChars(w, method, ml);
  WriteU32BE(w, body.len);
  IF body.len > 0 THEN
    AppendView(buf, body)
  END
END EncodeRequest;

PROCEDURE EncodeResponse(VAR buf: Buf;
                         requestId: CARDINAL;
                         body: BytesView);
VAR w: Writer;
BEGIN
  Clear(buf);
  InitWriter(w, buf);
  WriteHeader(w, MsgResponse, requestId);
  WriteU32BE(w, body.len);
  IF body.len > 0 THEN
    AppendView(buf, body)
  END
END EncodeResponse;

PROCEDURE EncodeError(VAR buf: Buf;
                      requestId: CARDINAL;
                      errCode: CARDINAL;
                      errMsg: ARRAY OF CHAR;
                      errMsgLen: CARDINAL;
                      body: BytesView);
VAR w: Writer; ml: CARDINAL;
BEGIN
  Clear(buf);
  InitWriter(w, buf);
  WriteHeader(w, MsgError, requestId);
  WriteU16BE(w, errCode);
  ml := errMsgLen;
  IF ml > HIGH(errMsg) + 1 THEN ml := HIGH(errMsg) + 1 END;
  WriteU16BE(w, ml);
  WriteChars(w, errMsg, ml);
  WriteU32BE(w, body.len);
  IF body.len > 0 THEN
    AppendView(buf, body)
  END
END EncodeError;

(* ── Decoding ─────────────────────────────────────────── *)

PROCEDURE DecodeHeader(payload: BytesView;
                       VAR version: CARDINAL;
                       VAR msgType: CARDINAL;
                       VAR requestId: CARDINAL;
                       VAR ok: BOOLEAN);
VAR r: Reader;
BEGIN
  ok := FALSE;
  InitReader(r, payload);
  version := ReadU8(r, ok);
  IF NOT ok THEN RETURN END;
  msgType := ReadU8(r, ok);
  IF NOT ok THEN RETURN END;
  requestId := ReadU32BE(r, ok)
END DecodeHeader;

PROCEDURE DecodeRequest(payload: BytesView;
                        VAR requestId: CARDINAL;
                        VAR method: BytesView;
                        VAR body: BytesView;
                        VAR ok: BOOLEAN);
VAR
  r: Reader;
  ver, mt, ml, bl: CARDINAL;
BEGIN
  ok := FALSE;
  method.base := NIL; method.len := 0;
  body.base := NIL; body.len := 0;
  InitReader(r, payload);

  (* Header *)
  ver := ReadU8(r, ok);  IF NOT ok THEN RETURN END;
  IF ver # Version THEN ok := FALSE; RETURN END;
  mt := ReadU8(r, ok);   IF NOT ok THEN RETURN END;
  IF mt # MsgRequest THEN ok := FALSE; RETURN END;
  requestId := ReadU32BE(r, ok);
  IF NOT ok THEN RETURN END;

  (* Method *)
  ml := ReadU16BE(r, ok); IF NOT ok THEN RETURN END;
  ReadSlice(r, ml, method, ok);
  IF NOT ok THEN RETURN END;

  (* Body *)
  bl := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
  IF bl = 0 THEN
    body.base := NIL;
    body.len := 0;
    ok := TRUE
  ELSE
    ReadSlice(r, bl, body, ok)
  END
END DecodeRequest;

PROCEDURE DecodeResponse(payload: BytesView;
                         VAR requestId: CARDINAL;
                         VAR body: BytesView;
                         VAR ok: BOOLEAN);
VAR
  r: Reader;
  ver, mt, bl: CARDINAL;
BEGIN
  ok := FALSE;
  body.base := NIL; body.len := 0;
  InitReader(r, payload);

  ver := ReadU8(r, ok);  IF NOT ok THEN RETURN END;
  IF ver # Version THEN ok := FALSE; RETURN END;
  mt := ReadU8(r, ok);   IF NOT ok THEN RETURN END;
  IF mt # MsgResponse THEN ok := FALSE; RETURN END;
  requestId := ReadU32BE(r, ok);
  IF NOT ok THEN RETURN END;

  bl := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
  IF bl = 0 THEN
    body.base := NIL;
    body.len := 0;
    ok := TRUE
  ELSE
    ReadSlice(r, bl, body, ok)
  END
END DecodeResponse;

PROCEDURE DecodeError(payload: BytesView;
                      VAR requestId: CARDINAL;
                      VAR errCode: CARDINAL;
                      VAR errMsg: BytesView;
                      VAR body: BytesView;
                      VAR ok: BOOLEAN);
VAR
  r: Reader;
  ver, mt, ml, bl: CARDINAL;
BEGIN
  ok := FALSE;
  errMsg.base := NIL; errMsg.len := 0;
  body.base := NIL; body.len := 0;
  InitReader(r, payload);

  ver := ReadU8(r, ok);  IF NOT ok THEN RETURN END;
  IF ver # Version THEN ok := FALSE; RETURN END;
  mt := ReadU8(r, ok);   IF NOT ok THEN RETURN END;
  IF mt # MsgError THEN ok := FALSE; RETURN END;
  requestId := ReadU32BE(r, ok);
  IF NOT ok THEN RETURN END;

  errCode := ReadU16BE(r, ok); IF NOT ok THEN RETURN END;
  ml := ReadU16BE(r, ok);      IF NOT ok THEN RETURN END;
  ReadSlice(r, ml, errMsg, ok);
  IF NOT ok THEN RETURN END;

  bl := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
  IF bl = 0 THEN
    body.base := NIL;
    body.len := 0;
    ok := TRUE
  ELSE
    ReadSlice(r, bl, body, ok)
  END
END DecodeError;

END RpcCodec.
