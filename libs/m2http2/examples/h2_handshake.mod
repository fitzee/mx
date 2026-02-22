MODULE h2_handshake;
(* Demonstrates HTTP/2 connection handshake:
   1. Client sends connection preface + SETTINGS
   2. Server sends SETTINGS (simulated)
   3. Client sends SETTINGS ACK
   4. Server sends SETTINGS ACK (simulated)
   5. Client opens a stream *)

FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM ByteBuf IMPORT BytesView, Buf, Init, Free, Clear, AsView,
                    AppendByte;
FROM Http2Types IMPORT FrameHeader, Settings, FrameSettings,
                       DefaultInitialWindowSize;
FROM Http2Frame IMPORT DecodeHeader, CheckPreface;
FROM Fsm IMPORT Fsm, Transition, StepStatus;
FROM Http2Stream IMPORT H2Stream, StreamTransTable;
FROM Http2Hpack IMPORT DynTable;
FROM Http2Conn IMPORT H2Conn, InitConn, FreeConn, SendPreface,
                      OpenStream, FindStream, ProcessFrame,
                      GetOutput, ClearOutput;
FROM Http2TestUtil IMPORT BuildSettingsFrame, BuildSettingsAckFrame,
                          ReadFrameHeader, ReadFramePayload;

PROCEDURE Main;
VAR c: H2Conn;
    serverBuf: Buf;
    v, payload: BytesView;
    hdr: FrameHeader;
    s: Settings;
    ok: BOOLEAN;
    sid: CARDINAL;
BEGIN
  (* 1. Initialise connection and send preface *)
  InitConn(c);
  SendPreface(c);
  v := GetOutput(c);
  WriteString("Client preface+SETTINGS: ");
  WriteCard(v.len, 0);
  WriteString(" bytes");
  WriteLn;

  IF CheckPreface(v) THEN
    WriteString("  Preface: valid")
  ELSE
    WriteString("  Preface: INVALID")
  END;
  WriteLn;
  ClearOutput(c);

  (* 2. Simulate server SETTINGS *)
  Init(serverBuf, 128);
  Http2Types.InitDefaultSettings(s);
  s.maxFrameSize := 32768;
  s.initialWindowSize := 131072;
  BuildSettingsFrame(serverBuf, s);
  v := AsView(serverBuf);
  ReadFrameHeader(v, hdr, ok);
  ReadFramePayload(v, hdr, payload, ok);
  ProcessFrame(c, hdr, payload, ok);
  IF ok THEN
    WriteString("Server SETTINGS processed: OK")
  ELSE
    WriteString("Server SETTINGS: ERROR")
  END;
  WriteLn;

  (* Client should have generated SETTINGS ACK *)
  v := GetOutput(c);
  WriteString("Client SETTINGS ACK: ");
  WriteCard(v.len, 0);
  WriteString(" bytes");
  WriteLn;
  ClearOutput(c);

  (* 3. Simulate server SETTINGS ACK *)
  Clear(serverBuf);
  BuildSettingsAckFrame(serverBuf);
  v := AsView(serverBuf);
  ReadFrameHeader(v, hdr, ok);
  ReadFramePayload(v, hdr, payload, ok);
  ProcessFrame(c, hdr, payload, ok);
  WriteString("Server SETTINGS ACK received: ");
  IF ok THEN WriteString("OK") ELSE WriteString("ERROR") END;
  WriteLn;

  (* 4. Open a stream *)
  sid := OpenStream(c);
  WriteString("Opened stream: ");
  WriteCard(sid, 0);
  WriteLn;

  WriteString("Remote maxFrameSize: ");
  WriteCard(c.remoteSettings.maxFrameSize, 0);
  WriteLn;
  WriteString("Remote initialWindowSize: ");
  WriteCard(c.remoteSettings.initialWindowSize, 0);
  WriteLn;

  Free(serverBuf);
  FreeConn(c);
  WriteString("Handshake complete.");
  WriteLn
END Main;

BEGIN
  Main
END h2_handshake.
