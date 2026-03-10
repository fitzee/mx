MODULE NestedModuleProc;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

  MODULE Inner;
    EXPORT Double;
    PROCEDURE Double(x: INTEGER): INTEGER;
    BEGIN
      RETURN x * 2;
    END Double;
  END Inner;

BEGIN
  WriteInt(Double(21), 0);
  WriteLn;
END NestedModuleProc.
