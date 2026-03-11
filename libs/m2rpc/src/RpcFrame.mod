IMPLEMENTATION MODULE RpcFrame;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear,
                     AppendByte, AppendChars, AppendView,
                     Reserve, AsView;

CONST
  StReadLen     = 0;
  StReadPayload = 1;
  ChunkSize     = 1024;

(* ── Helpers ──────────────────────────────────────────── *)

PROCEDURE DecodeBE32(VAR a: ARRAY OF CHAR): CARDINAL;
BEGIN
  RETURN ORD(a[0]) * 16777216
       + ORD(a[1]) * 65536
       + ORD(a[2]) * 256
       + ORD(a[3])
END DecodeBE32;

PROCEDURE EncodeBE32(val: CARDINAL; VAR a: ARRAY OF CHAR);
BEGIN
  a[0] := CHR((val DIV 16777216) MOD 256);
  a[1] := CHR((val DIV 65536) MOD 256);
  a[2] := CHR((val DIV 256) MOD 256);
  a[3] := CHR(val MOD 256)
END EncodeBE32;

(* ── Transport call helpers ───────────────────────────── *)
(* Calling through procedure-typed variables requires the
   codegen to know about VAR parameters.  Declaring a local
   variable of the proc type causes gen_var_decl to register
   the parameter info, enabling correct & emission. *)

PROCEDURE CallRead(fn: ReadFn; ctx: ADDRESS;
                   buf: ADDRESS; max: CARDINAL;
                   VAR got: CARDINAL): CARDINAL;
VAR doRead: ReadFn;
BEGIN
  doRead := fn;
  RETURN doRead(ctx, buf, max, got)
END CallRead;

PROCEDURE CallWrite(fn: WriteFn; ctx: ADDRESS;
                    buf: ADDRESS; len: CARDINAL;
                    VAR sent: CARDINAL): CARDINAL;
VAR doWrite: WriteFn;
BEGIN
  doWrite := fn;
  RETURN doWrite(ctx, buf, len, sent)
END CallWrite;

(* ── FrameReader ──────────────────────────────────────── *)

PROCEDURE InitFrameReader(VAR fr: FrameReader;
                          maxFrame: CARDINAL;
                          fn: ReadFn; ctx: ADDRESS);
BEGIN
  fr.state := StReadLen;
  fr.lenPos := 0;
  fr.payloadLen := 0;
  fr.payloadPos := 0;
  fr.readFn := fn;
  fr.readCtx := ctx;
  IF maxFrame > MaxFrame THEN
    fr.maxFrame := MaxFrame
  ELSE
    fr.maxFrame := maxFrame
  END;
  Init(fr.payloadBuf, 256)
END InitFrameReader;

PROCEDURE TryReadFrame(VAR fr: FrameReader;
                       VAR out: BytesView;
                       VAR status: FrameStatus);
VAR
  tmp: ARRAY [0..ChunkSize-1] OF CHAR;
  want, got, ts, i: CARDINAL;
  rfn: ReadFn;
  rctx: ADDRESS;
BEGIN
  out.base := NIL;
  out.len := 0;

  rfn := fr.readFn;
  rctx := fr.readCtx;

  (* Phase 1: read the 4-byte length prefix *)
  IF fr.state = StReadLen THEN
    WHILE fr.lenPos < 4 DO
      want := 4 - fr.lenPos;
      ts := CallRead(rfn, rctx, ADR(tmp), want, got);
      IF ts = TsClosed THEN
        IF fr.lenPos = 0 THEN
          status := FrmClosed
        ELSE
          status := FrmError
        END;
        RETURN
      ELSIF ts = TsWouldBlock THEN
        status := FrmNeedMore;
        RETURN
      ELSIF ts = TsError THEN
        status := FrmError;
        RETURN
      END;
      IF got = 0 THEN
        status := FrmNeedMore;
        RETURN
      END;
      i := 0;
      WHILE i < got DO
        fr.lenBuf[fr.lenPos] := tmp[i];
        INC(fr.lenPos);
        INC(i)
      END
    END;

    fr.payloadLen := DecodeBE32(fr.lenBuf);
    IF fr.payloadLen > fr.maxFrame THEN
      status := FrmTooLarge;
      RETURN
    END;

    IF fr.payloadLen = 0 THEN
      Clear(fr.payloadBuf);
      out := AsView(fr.payloadBuf);
      fr.lenPos := 0;
      status := FrmOk;
      RETURN
    END;

    Clear(fr.payloadBuf);
    IF NOT Reserve(fr.payloadBuf, fr.payloadLen) THEN
      status := FrmError;
      RETURN
    END;
    fr.payloadPos := 0;
    fr.state := StReadPayload
  END;

  (* Phase 2: read payload bytes *)
  WHILE fr.payloadPos < fr.payloadLen DO
    want := fr.payloadLen - fr.payloadPos;
    IF want > ChunkSize THEN want := ChunkSize END;
    ts := CallRead(rfn, rctx, ADR(tmp), want, got);
    IF ts = TsClosed THEN
      status := FrmError;
      RETURN
    ELSIF ts = TsWouldBlock THEN
      status := FrmNeedMore;
      RETURN
    ELSIF ts = TsError THEN
      status := FrmError;
      RETURN
    END;
    IF got = 0 THEN
      status := FrmNeedMore;
      RETURN
    END;
    i := 0;
    WHILE i < got DO
      AppendByte(fr.payloadBuf, ORD(tmp[i]));
      INC(i)
    END;
    fr.payloadPos := fr.payloadPos + got
  END;

  out := AsView(fr.payloadBuf);
  fr.state := StReadLen;
  fr.lenPos := 0;
  status := FrmOk
END TryReadFrame;

PROCEDURE ResetFrameReader(VAR fr: FrameReader);
BEGIN
  fr.state := StReadLen;
  fr.lenPos := 0;
  fr.payloadLen := 0;
  fr.payloadPos := 0;
  Clear(fr.payloadBuf)
END ResetFrameReader;

PROCEDURE FreeFrameReader(VAR fr: FrameReader);
BEGIN
  Free(fr.payloadBuf)
END FreeFrameReader;

(* ── WriteFrame ───────────────────────────────────────── *)

PROCEDURE WriteFrame(fn: WriteFn; ctx: ADDRESS;
                     payload: BytesView;
                     VAR ok: BOOLEAN);
VAR
  frameBuf: Buf;
  hdr: ARRAY [0..3] OF CHAR;
  view: BytesView;
  pos, sent, ts: CARDINAL;
BEGIN
  ok := FALSE;

  (* Build complete frame: header + payload *)
  Init(frameBuf, payload.len + 4);
  EncodeBE32(payload.len, hdr);
  AppendChars(frameBuf, hdr, 4);
  IF payload.len > 0 THEN
    AppendView(frameBuf, payload)
  END;

  view := AsView(frameBuf);
  pos := 0;
  WHILE pos < view.len DO
    ts := CallWrite(fn, ctx, VAL(ADDRESS, LONGCARD(view.base) + LONGCARD(pos)), view.len - pos, sent);
    IF (ts = TsError) OR (ts = TsClosed) THEN
      Free(frameBuf);
      RETURN
    END;
    IF (ts # TsWouldBlock) AND (sent > 0) THEN
      pos := pos + sent
    END
  END;

  Free(frameBuf);
  ok := TRUE
END WriteFrame;

END RpcFrame.
