MODULE NestedProcCollision;
FROM InOut IMPORT WriteInt, WriteLn;

PROCEDURE Alpha;
  PROCEDURE Helper(): INTEGER;
  BEGIN RETURN 1; END Helper;
BEGIN
  WriteInt(Helper(), 0);
  WriteLn;
END Alpha;

PROCEDURE Beta;
  PROCEDURE Helper(): INTEGER;
  BEGIN RETURN 2; END Helper;
BEGIN
  WriteInt(Helper(), 0);
  WriteLn;
END Beta;

BEGIN
  Alpha;
  Beta;
END NestedProcCollision.
