MODULE ForCharLiteral;
(* Regression: single-char string literals in FOR loop bounds were emitted
   as C string literals ("A") instead of char literals ('A'), causing
   incompatible pointer-to-integer conversion errors. *)

FROM InOut IMPORT WriteChar, WriteString, WriteLn;

VAR
    ch : CHAR;
    count : CARDINAL;

PROCEDURE CountRange(lo, hi : CHAR) : CARDINAL;
VAR c : CHAR;
    n : CARDINAL;
BEGIN
    n := 0;
    FOR c := lo TO hi DO
        INC(n)
    END;
    RETURN n
END CountRange;

BEGIN
    (* FOR loop with char literal bounds *)
    FOR ch := 'A' TO 'E' DO
        WriteChar(ch)
    END;
    WriteLn;

    (* Char literal passed to procedure and used in FOR *)
    count := CountRange('A', 'Z');
    IF count = 26 THEN
        WriteString("alpha=26")
    ELSE
        WriteString("WRONG")
    END;
    WriteLn;

    (* Single char 'Z' to 'Z' — one iteration *)
    count := 0;
    FOR ch := 'Z' TO 'Z' DO
        INC(count)
    END;
    WriteString("single=");
    IF count = 1 THEN
        WriteString("1")
    ELSE
        WriteString("WRONG")
    END;
    WriteLn
END ForCharLiteral.
