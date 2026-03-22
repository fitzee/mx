IMPLEMENTATION MODULE NetB;
FROM NetA IMPORT Status, Ok, Error;
FROM Strings IMPORT Assign;

TYPE
  (* impl-only types — SAME NAMES as NetA's impl-only types *)
  ConnRec = RECORD
    fd: INTEGER;
    localPort: INTEGER;
    remotePort: INTEGER;
    connected: BOOLEAN;
  END;
  ConnPtr = POINTER TO ConnRec;

PROCEDURE Connect(VAR ep: Endpoint): Status;
BEGIN
  ep.status := Ok;
  RETURN Ok
END Connect;

(* impl-only: uses the local ConnPtr, NOT NetA's *)
PROCEDURE Disconnect(c: ConnPtr);
BEGIN
  c^.connected := FALSE
END Disconnect;

END NetB.
