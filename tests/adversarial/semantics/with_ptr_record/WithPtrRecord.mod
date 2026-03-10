MODULE WithPtrRecord;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  RecPtr = POINTER TO RECORD
    x: INTEGER;
    y: INTEGER;
  END;

VAR
  p: RecPtr;

BEGIN
  NEW(p);
  p^.x := 42;
  p^.y := 99;
  WITH p^ DO
    WriteInt(x, 0);
    WriteLn;
    WriteInt(y, 0);
    WriteLn;
  END;
  DISPOSE(p);
END WithPtrRecord.
