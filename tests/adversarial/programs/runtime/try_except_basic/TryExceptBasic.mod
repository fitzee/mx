MODULE TryExceptBasic;
(* Tests basic TRY/EXCEPT: body runs once, handler runs once on RAISE,
   code after TRY/EXCEPT runs, and messages don't duplicate. *)
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

EXCEPTION Alpha;
EXCEPTION Beta;

VAR count: INTEGER;

PROCEDURE RaiseAlpha;
BEGIN
  WriteString("raise"); WriteLn;
  RAISE Alpha
END RaiseAlpha;

PROCEDURE RaiseBeta;
BEGIN
  RAISE Beta
END RaiseBeta;

BEGIN
  count := 0;

  (* Test 1: named handler catches correct exception *)
  TRY
    INC(count);
    RaiseAlpha;
    WriteString("FAIL-unreachable"); WriteLn
  EXCEPT Alpha DO
    WriteString("caught-alpha"); WriteLn
  END;

  (* Test 2: catch-all handler *)
  TRY
    INC(count);
    RaiseBeta;
    WriteString("FAIL-unreachable2"); WriteLn
  EXCEPT
    WriteString("caught-all"); WriteLn
  END;

  (* Test 3: no exception — body runs, handler skipped *)
  TRY
    INC(count);
    WriteString("no-raise"); WriteLn
  EXCEPT Alpha DO
    WriteString("FAIL-spurious"); WriteLn
  END;

  (* count should be exactly 3 — each TRY body ran once *)
  WriteString("count="); WriteInt(count, 1); WriteLn
END TryExceptBasic.
