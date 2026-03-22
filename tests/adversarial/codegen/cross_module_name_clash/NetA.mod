IMPLEMENTATION MODULE NetA;
FROM Storage IMPORT ALLOCATE;
FROM SYSTEM IMPORT TSIZE;
FROM Strings IMPORT Assign;

TYPE
  (* impl-only types — same names as NetB *)
  ConnRec = RECORD
    id: INTEGER;
    resp: ResponsePtr;
    reqLen: INTEGER;
    active: BOOLEAN;
  END;
  ConnPtr = POINTER TO ConnRec;

PROCEDURE MakeResponse(code: INTEGER): ResponsePtr;
VAR r: ResponsePtr;
BEGIN
  ALLOCATE(r, TSIZE(Response));
  r^.code := code;
  r^.headerCount := 0;
  RETURN r
END MakeResponse;

PROCEDURE AddHeader(c: ConnPtr; name: ARRAY OF CHAR; val: INTEGER);
BEGIN
  IF c^.resp^.headerCount < 4 THEN
    Assign(name, c^.resp^.headers[c^.resp^.headerCount].name);
    c^.resp^.headers[c^.resp^.headerCount].value := val;
    INC(c^.resp^.headerCount)
  END
END AddHeader;

PROCEDURE GetHeaderCount(c: ConnPtr): INTEGER;
BEGIN
  RETURN c^.resp^.headerCount
END GetHeaderCount;

END NetA.
