IMPLEMENTATION MODULE SWholeIO;
FROM CIO IMPORT getchar, putchar;

(* Reuse InOut's formatting logic via direct implementation *)

PROCEDURE WriteUnsigned(val: CARDINAL; w: CARDINAL);
VAR
    buf: ARRAY [0..11] OF CHAR;
    i, len: CARDINAL;
    v: CARDINAL;
BEGIN
    IF val = 0 THEN
        buf[0] := '0'; len := 1
    ELSE
        len := 0; v := val;
        WHILE v > 0 DO
            buf[len] := CHR(ORD('0') + (v MOD 10));
            v := v DIV 10;
            INC(len)
        END
    END;
    WHILE w > len DO putchar(ORD(' ')); DEC(w) END;
    i := len;
    WHILE i > 0 DO DEC(i); putchar(ORD(buf[i])) END
END WriteUnsigned;

PROCEDURE WriteInt(n: INTEGER; w: CARDINAL);
VAR u, digs, v: CARDINAL;
BEGIN
    IF n < 0 THEN
        IF w > 0 THEN DEC(w) END;
        IF n = MIN(INTEGER) THEN
            u := CARDINAL(MAX(INTEGER)) + 1
        ELSE
            u := CARDINAL(-n)
        END;
        digs := 0; v := u;
        IF v = 0 THEN digs := 1
        ELSE
            WHILE v > 0 DO INC(digs); v := v DIV 10 END
        END;
        WHILE w > digs DO putchar(ORD(' ')); DEC(w) END;
        putchar(ORD('-'));
        WriteUnsigned(u, 0)
    ELSE
        WriteUnsigned(CARDINAL(n), w)
    END
END WriteInt;

PROCEDURE ReadInt(VAR n: INTEGER);
VAR c: INTEGER; neg: BOOLEAN; val: INTEGER;
BEGIN
    neg := FALSE;
    c := getchar();
    WHILE (c = ORD(' ')) OR (c = 9) OR (c = 10) OR (c = 13) DO
        c := getchar()
    END;
    IF c = ORD('-') THEN neg := TRUE; c := getchar()
    ELSIF c = ORD('+') THEN c := getchar()
    END;
    val := 0;
    WHILE (c >= ORD('0')) AND (c <= ORD('9')) DO
        val := val * 10 + (c - ORD('0'));
        c := getchar()
    END;
    IF neg THEN n := -val ELSE n := val END
END ReadInt;

PROCEDURE WriteCard(n: CARDINAL; w: CARDINAL);
BEGIN
    WriteUnsigned(n, w)
END WriteCard;

PROCEDURE ReadCard(VAR n: CARDINAL);
VAR c: INTEGER; val: CARDINAL;
BEGIN
    c := getchar();
    WHILE (c = ORD(' ')) OR (c = 9) OR (c = 10) OR (c = 13) DO
        c := getchar()
    END;
    val := 0;
    WHILE (c >= ORD('0')) AND (c <= ORD('9')) DO
        val := val * 10 + CARDINAL(c - ORD('0'));
        c := getchar()
    END;
    n := val
END ReadCard;

END SWholeIO.
