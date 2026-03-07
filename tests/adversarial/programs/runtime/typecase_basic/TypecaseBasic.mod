MODULE TypecaseBasic;
(*
 * Test TYPECASE with REF types — basic branch dispatch, ELSE, and variable binding.
 * Uses --m2plus for REF/REFANY/TYPECASE.
 *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;

TYPE
    IntRef = REF INTEGER;
    RealRef = REF REAL;
    CharRef = REF CHAR;

VAR
    ri: IntRef;
    rr: RealRef;
    rc: CharRef;

PROCEDURE Classify(r: REFANY);
BEGIN
    TYPECASE r OF
      IntRef:
        WriteString("int");
        WriteLn
    | RealRef:
        WriteString("real");
        WriteLn
    | CharRef:
        WriteString("char");
        WriteLn
    ELSE
        WriteString("other");
        WriteLn
    END;
END Classify;

PROCEDURE ClassifyBind(r: REFANY);
VAR
    val: INTEGER;
BEGIN
    TYPECASE r OF
      IntRef (x):
        val := x^;
        WriteString("int=");
        WriteInt(val, 0);
        WriteLn
    | RealRef:
        WriteString("real");
        WriteLn
    ELSE
        WriteString("else");
        WriteLn
    END;
END ClassifyBind;

BEGIN
    NEW(ri);
    ri^ := 42;
    NEW(rr);
    rr^ := 3.14;
    NEW(rc);
    rc^ := 'A';

    (* Test basic dispatch *)
    Classify(ri);
    Classify(rr);
    Classify(rc);

    (* Test ELSE branch with NIL *)
    Classify(NIL);

    (* Test variable binding *)
    ClassifyBind(ri);
    ClassifyBind(rr);
    ClassifyBind(rc);
END TypecaseBasic.
