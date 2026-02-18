MODULE M2PlusObjects;
(* Test Modula-2+ OBJECT types *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Shape = OBJECT
    x, y: INTEGER;
  METHODS
    Area(): INTEGER;
    Describe();
  END;

BEGIN
  WriteString("=== M2+ OBJECT Type Test ==="); WriteLn;
  WriteString("Object types parsed successfully"); WriteLn;
  WriteString("Done"); WriteLn
END M2PlusObjects.
