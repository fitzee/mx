IMPLEMENTATION MODULE Http2TestUtil;

FROM ByteBuf IMPORT Buf, BytesView, AppendByte, AppendView,
                    ViewGetByte, AsView;
FROM Codec IMPORT Reader, InitReader, ReadU8, ReadU32BE, ReadSlice,
                  Remaining;
FROM Http2Types IMPORT FrameHeader, Settings, FrameHeaderSize,
                       FrameSettings, FramePing, FrameGoaway,
                       FrameWindowUpdate, FrameRstStream,
                       FlagAck, ConnectionStreamId,
                       SetHeaderTableSize, SetEnablePush,
                       SetMaxConcurrentStreams, SetInitialWindowSize,
                       SetMaxFrameSize, SetMaxHeaderListSize;
FROM Http2Frame IMPORT EncodeHeader, EncodeSettings, EncodeSettingsAck,
                       EncodePing, EncodeGoaway, EncodeWindowUpdate,
                       EncodeRstStream;

(* ── Frame builder ─────────────────────────────────────── *)

PROCEDURE BuildFrame(VAR b: Buf; ftype: CARDINAL; flags: CARDINAL;
                     streamId: CARDINAL; payload: BytesView);
VAR hdr: FrameHeader;
BEGIN
  hdr.length := payload.len;
  hdr.ftype := ftype;
  hdr.flags := flags;
  hdr.streamId := streamId;
  EncodeHeader(b, hdr);
  IF payload.len > 0 THEN
    AppendView(b, payload)
  END
END BuildFrame;

PROCEDURE BuildSettingsFrame(VAR b: Buf; s: Settings);
BEGIN
  EncodeSettings(b, s)
END BuildSettingsFrame;

PROCEDURE BuildSettingsAckFrame(VAR b: Buf);
BEGIN
  EncodeSettingsAck(b)
END BuildSettingsAckFrame;

PROCEDURE BuildPingFrame(VAR b: Buf; data: BytesView; isAck: BOOLEAN);
BEGIN
  EncodePing(b, data, isAck)
END BuildPingFrame;

PROCEDURE BuildGoawayFrame(VAR b: Buf; lastStreamId: CARDINAL;
                           errorCode: CARDINAL);
BEGIN
  EncodeGoaway(b, lastStreamId, errorCode)
END BuildGoawayFrame;

PROCEDURE BuildWindowUpdateFrame(VAR b: Buf; streamId: CARDINAL;
                                 increment: CARDINAL);
BEGIN
  EncodeWindowUpdate(b, streamId, increment)
END BuildWindowUpdateFrame;

PROCEDURE BuildRstStreamFrame(VAR b: Buf; streamId: CARDINAL;
                              errorCode: CARDINAL);
BEGIN
  EncodeRstStream(b, streamId, errorCode)
END BuildRstStreamFrame;

(* ── Frame reader ──────────────────────────────────────── *)

PROCEDURE ReadFrameHeader(VAR v: BytesView; VAR hdr: FrameHeader;
                          VAR ok: BOOLEAN);
VAR r: Reader;
    b0, b1, b2, raw32: CARDINAL;
BEGIN
  ok := TRUE;
  IF v.len < FrameHeaderSize THEN ok := FALSE; RETURN END;
  InitReader(r, v);
  b0 := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  b1 := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  b2 := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  hdr.length := b0 * 65536 + b1 * 256 + b2;
  hdr.ftype := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  hdr.flags := ReadU8(r, ok); IF NOT ok THEN RETURN END;
  raw32 := ReadU32BE(r, ok); IF NOT ok THEN RETURN END;
  hdr.streamId := raw32 MOD 2147483648;
  (* Advance the view past the header *)
  v.base := VAL(ADDRESS, LONGCARD(v.base) + LONGCARD(FrameHeaderSize));
  v.len := v.len - FrameHeaderSize
END ReadFrameHeader;

PROCEDURE ReadFramePayload(VAR v: BytesView; hdr: FrameHeader;
                           VAR payload: BytesView;
                           VAR ok: BOOLEAN);
BEGIN
  ok := TRUE;
  IF v.len < hdr.length THEN ok := FALSE; RETURN END;
  payload.base := v.base;
  payload.len := hdr.length;
  v.base := VAL(ADDRESS, LONGCARD(v.base) + LONGCARD(hdr.length));
  v.len := v.len - hdr.length
END ReadFramePayload;

END Http2TestUtil.
