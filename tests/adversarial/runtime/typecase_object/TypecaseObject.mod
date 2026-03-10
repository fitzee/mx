MODULE TypecaseObject;
(*
 * Test TYPECASE with OBJECT types and subtype matching via M2_ISA.
 * OBJECT types are reference types — use . not ^. for field access.
 *)

FROM InOut IMPORT WriteString, WriteLn;

TYPE
    Shape = OBJECT
        x, y: INTEGER;
    END;

    Circle = Shape OBJECT
        radius: INTEGER;
    END;

    Square = Shape OBJECT
        side: INTEGER;
    END;

VAR
    s: Shape;
    c: Circle;
    q: Square;

PROCEDURE Identify(obj: REFANY);
BEGIN
    TYPECASE obj OF
      Circle:
        WriteString("circle");
        WriteLn
    | Square:
        WriteString("square");
        WriteLn
    | Shape:
        WriteString("shape");
        WriteLn
    ELSE
        WriteString("unknown");
        WriteLn
    END;
END Identify;

BEGIN
    NEW(c);
    NEW(q);
    NEW(s);

    (* Circle is checked first, so it matches Circle, not Shape *)
    Identify(c);
    (* Square matches Square *)
    Identify(q);
    (* Plain Shape only matches Shape *)
    Identify(s);
    (* NIL matches ELSE *)
    Identify(NIL);
END TypecaseObject.
