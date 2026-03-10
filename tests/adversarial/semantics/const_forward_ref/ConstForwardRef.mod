MODULE ConstForwardRef;
FROM InOut IMPORT WriteInt, WriteLn;

CONST
  Total = Base + Extra;
  Base = 10;
  Extra = 5;

BEGIN
  WriteInt(Total, 0);
  WriteLn;
  WriteInt(Base, 0);
  WriteLn;
END ConstForwardRef.
