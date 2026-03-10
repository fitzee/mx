MODULE TryExceptNested;
(* Tests nested TRY/EXCEPT: inner catches its own, outer catches unhandled,
   and FINALLY always runs. *)
FROM InOut IMPORT WriteString, WriteLn;

EXCEPTION Inner;
EXCEPTION Outer;

PROCEDURE RaiseInner;
BEGIN
  RAISE Inner
END RaiseInner;

PROCEDURE RaiseOuter;
BEGIN
  RAISE Outer
END RaiseOuter;

BEGIN
  (* Test 1: inner catches inner, outer never triggered *)
  TRY
    WriteString("outer-body"); WriteLn;
    TRY
      WriteString("inner-body"); WriteLn;
      RaiseInner
    EXCEPT Inner DO
      WriteString("inner-caught"); WriteLn
    END;
    WriteString("outer-continues"); WriteLn
  EXCEPT Outer DO
    WriteString("FAIL-outer-caught"); WriteLn
  END;

  (* Test 2: inner doesn't handle, outer catches *)
  TRY
    TRY
      RaiseOuter
    EXCEPT Inner DO
      WriteString("FAIL-wrong-handler"); WriteLn
    END;
    WriteString("FAIL-should-not-reach"); WriteLn
  EXCEPT Outer DO
    WriteString("outer-caught"); WriteLn
  END;

  (* Test 3: FINALLY runs on normal path *)
  TRY
    WriteString("try-body"); WriteLn
  FINALLY
    WriteString("finally-normal"); WriteLn
  END;

  (* Test 4: FINALLY runs on exception path *)
  TRY
    TRY
      RaiseInner
    FINALLY
      WriteString("finally-exc"); WriteLn
    END
  EXCEPT Inner DO
    WriteString("outer-after-finally"); WriteLn
  END;

  WriteString("done"); WriteLn
END TryExceptNested.
