MODULE FFITest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM CAdd IMPORT add, multiply;
BEGIN
  WriteString("3 + 4 = "); WriteInt(add(3, 4), 1); WriteLn;
  WriteString("3 * 4 = "); WriteInt(multiply(3, 4), 1); WriteLn
END FFITest.
