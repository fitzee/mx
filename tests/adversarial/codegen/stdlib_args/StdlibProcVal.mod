MODULE StdlibProcVal;
(* Regression: passing a stdlib procedure (Args.GetArg) as a procedure
   value to another procedure causes the LLVM backend to emit a load
   from @Args_GetArg instead of passing it as a function pointer. *)

FROM Args IMPORT ArgCount, GetArg;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  GetArgProc = PROCEDURE(CARDINAL, VAR ARRAY OF CHAR);

PROCEDURE UseProc(n: CARDINAL; getter: GetArgProc);
VAR arg: ARRAY [0..255] OF CHAR;
BEGIN
  IF n >= 1 THEN
    getter(0, arg);
    WriteString("got=");
    WriteString(arg);
    WriteLn
  END
END UseProc;

BEGIN
  WriteString("argc=");
  WriteInt(ArgCount(), 0);
  WriteLn;
  UseProc(ArgCount(), GetArg)
END StdlibProcVal.
