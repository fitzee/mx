IMPLEMENTATION MODULE PtrRec;
FROM SYSTEM IMPORT ADDRESS;

TYPE BytePtr = POINTER TO CHAR;

PROCEDURE InitPtr(VAR r: PtrBuf; p: ADDRESS; sz: INTEGER);
BEGIN
  r.data := p;
  r.size := sz
END InitPtr;

PROCEDURE ReadByte(VAR r: PtrBuf; idx: INTEGER): INTEGER;
VAR bp: BytePtr;
    ch: CHAR;
BEGIN
  IF (idx < 0) OR (idx >= r.size) THEN RETURN -1 END;
  bp := r.data + idx;
  ch := bp^;
  RETURN ORD(ch) MOD 256
END ReadByte;

END PtrRec.
