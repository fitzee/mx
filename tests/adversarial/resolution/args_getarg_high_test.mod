MODULE args_getarg_high_test;
(* Test that Args.GetArg correctly passes the open-array HIGH parameter.
   Without the fix, the generated C code was missing the HIGH argument.
   With the fix, the call includes the buffer size as the third argument. *)
FROM Args IMPORT ArgCount, GetArg;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;

VAR
  buf: ARRAY [0..63] OF CHAR;
  n: CARDINAL;

BEGIN
  n := ArgCount();
  IF n >= 1 THEN
    GetArg(0, buf);
    WriteString("arg0: "); WriteString(buf); WriteLn
  END;
  WriteString("args_getarg_high OK"); WriteLn
END args_getarg_high_test.
