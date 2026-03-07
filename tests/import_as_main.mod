MODULE ImportAsMain;

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM ImportAsMulti IMPORT Greet AS SayHi, Add AS Plus;

BEGIN
  (* Test aliased imports from user module *)
  SayHi("Hello via alias");

  (* Test aliased function call *)
  WriteString("Sum: ");
  WriteInt(Plus(10, 32), 1);
  WriteLn;

  (* Mix aliased and non-aliased *)
  WriteString("Direct: ");
  WriteInt(Plus(1, 2), 1);
  WriteLn;
END ImportAsMain.
