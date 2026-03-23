IMPLEMENTATION MODULE InOut;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM CIO IMPORT getchar, putchar;

VAR Done: BOOLEAN;

(* ── Output ────────────────────────────────────────── *)

PROCEDURE WriteString(s: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
    i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # CHR(0)) DO
        putchar(ORD(s[i]));
        INC(i)
    END
END WriteString;

PROCEDURE WriteLn;
BEGIN
    putchar(10)
END WriteLn;

PROCEDURE Write(ch: CHAR);
BEGIN
    putchar(ORD(ch))
END Write;

PROCEDURE WriteChar(ch: CHAR);
BEGIN
    putchar(ORD(ch))
END WriteChar;

(* Helper: write an unsigned decimal with minimum width w *)
PROCEDURE WriteUnsigned(val: CARDINAL; w: CARDINAL);
VAR
    buf: ARRAY [0..11] OF CHAR;
    i, len: CARDINAL;
    v: CARDINAL;
BEGIN
    (* convert to digits in reverse *)
    IF val = 0 THEN
        buf[0] := '0';
        len := 1
    ELSE
        len := 0;
        v := val;
        WHILE v > 0 DO
            buf[len] := CHR(ORD('0') + (v MOD 10));
            v := v DIV 10;
            INC(len)
        END
    END;
    (* pad with spaces *)
    WHILE w > len DO
        putchar(ORD(' '));
        DEC(w)
    END;
    (* output digits in correct order *)
    i := len;
    WHILE i > 0 DO
        DEC(i);
        putchar(ORD(buf[i]))
    END
END WriteUnsigned;

PROCEDURE WriteIntHelper(n: INTEGER; w: CARDINAL; neg: BOOLEAN);
(* Write |n| as digits preceded by '-' if neg, padded to width w.
   Handles MIN(INTEGER) by writing last digit separately. *)
VAR
    buf: ARRAY [0..11] OF CHAR;
    len, i: CARDINAL;
    v: INTEGER;
    lastDigit: CARDINAL;
BEGIN
    (* Extract digits from |n| into buf (reversed) *)
    IF n = 0 THEN
        buf[0] := '0'; len := 1
    ELSIF n = MIN(INTEGER) THEN
        (* MIN(INTEGER) = -2147483648; can't negate to get positive value.
           PIM4 floored: (-2147483648) MOD 10 = 2, DIV 10 = -214748365.
           Absolute value decomposition: |n| = (-(n DIV 10) - 1) * 10 + (10 - n MOD 10)
           when n MOD 10 > 0. *)
        lastDigit := CARDINAL(n MOD 10);
        IF lastDigit > 0 THEN
            lastDigit := 10 - lastDigit;
            v := -(n DIV 10) - 1
        ELSE
            lastDigit := 0;
            v := -(n DIV 10)
        END;
        buf[0] := CHR(ORD('0') + lastDigit);
        len := 1;
        WHILE v > 0 DO
            buf[len] := CHR(ORD('0') + CARDINAL(v MOD 10));
            v := v DIV 10;
            INC(len)
        END
    ELSE
        len := 0;
        IF n < 0 THEN v := -n ELSE v := n END;
        WHILE v > 0 DO
            buf[len] := CHR(ORD('0') + CARDINAL(v MOD 10));
            v := v DIV 10;
            INC(len)
        END
    END;
    (* total chars = len + (1 if neg) *)
    IF neg THEN
        WHILE w > len + 1 DO putchar(ORD(' ')); DEC(w) END;
        putchar(ORD('-'))
    ELSE
        WHILE w > len DO putchar(ORD(' ')); DEC(w) END
    END;
    i := len;
    WHILE i > 0 DO DEC(i); putchar(ORD(buf[i])) END
END WriteIntHelper;

PROCEDURE WriteInt(n: INTEGER; w: CARDINAL);
BEGIN
    IF n < 0 THEN
        WriteIntHelper(n, w, TRUE)
    ELSE
        WriteIntHelper(n, w, FALSE)
    END
END WriteInt;

PROCEDURE WriteCard(n: CARDINAL; w: CARDINAL);
BEGIN
    WriteUnsigned(n, w)
END WriteCard;

PROCEDURE WriteHex(n: CARDINAL; w: CARDINAL);
VAR
    buf: ARRAY [0..7] OF CHAR;
    i, len: CARDINAL;
    d: CARDINAL;
BEGIN
    IF n = 0 THEN
        buf[0] := '0'; len := 1
    ELSE
        len := 0;
        WHILE n > 0 DO
            d := n MOD 16;
            IF d < 10 THEN
                buf[len] := CHR(ORD('0') + d)
            ELSE
                buf[len] := CHR(ORD('A') + d - 10)
            END;
            n := n DIV 16;
            INC(len)
        END
    END;
    WHILE w > len DO putchar(ORD(' ')); DEC(w) END;
    i := len;
    WHILE i > 0 DO DEC(i); putchar(ORD(buf[i])) END
END WriteHex;

PROCEDURE WriteOct(n: CARDINAL; w: CARDINAL);
VAR
    buf: ARRAY [0..11] OF CHAR;
    i, len: CARDINAL;
BEGIN
    IF n = 0 THEN
        buf[0] := '0'; len := 1
    ELSE
        len := 0;
        WHILE n > 0 DO
            buf[len] := CHR(ORD('0') + (n MOD 8));
            n := n DIV 8;
            INC(len)
        END
    END;
    WHILE w > len DO putchar(ORD(' ')); DEC(w) END;
    i := len;
    WHILE i > 0 DO DEC(i); putchar(ORD(buf[i])) END
END WriteOct;

(* ── Input ─────────────────────────────────────────── *)

PROCEDURE Read(VAR ch: CHAR);
VAR c: INTEGER;
BEGIN
    c := getchar();
    IF c = -1 THEN
        ch := CHR(0);
        Done := FALSE
    ELSE
        ch := CHR(c);
        Done := TRUE
    END
END Read;

PROCEDURE ReadChar(VAR ch: CHAR);
BEGIN
    Read(ch)
END ReadChar;

PROCEDURE ReadString(VAR s: ARRAY OF CHAR);
VAR c: INTEGER; i: CARDINAL;
BEGIN
    (* skip leading whitespace *)
    c := getchar();
    WHILE (c = ORD(' ')) OR (c = 9) OR (c = 10) OR (c = 13) DO
        c := getchar()
    END;
    IF c = -1 THEN
        s[0] := CHR(0);
        Done := FALSE;
        RETURN
    END;
    i := 0;
    WHILE (c # -1) AND (c # ORD(' ')) AND (c # 9)
          AND (c # 10) AND (c # 13) AND (i <= HIGH(s)) DO
        s[i] := CHR(c);
        INC(i);
        c := getchar()
    END;
    IF i <= HIGH(s) THEN s[i] := CHR(0) END;
    Done := TRUE
END ReadString;

PROCEDURE ReadInt(VAR n: INTEGER);
VAR
    c: INTEGER;
    neg: BOOLEAN;
    val: INTEGER;
BEGIN
    neg := FALSE;
    (* skip whitespace *)
    c := getchar();
    WHILE (c = ORD(' ')) OR (c = 9) OR (c = 10) OR (c = 13) DO
        c := getchar()
    END;
    IF c = -1 THEN Done := FALSE; n := 0; RETURN END;
    IF c = ORD('-') THEN neg := TRUE; c := getchar()
    ELSIF c = ORD('+') THEN c := getchar()
    END;
    IF (c < ORD('0')) OR (c > ORD('9')) THEN
        Done := FALSE; n := 0; RETURN
    END;
    val := 0;
    WHILE (c >= ORD('0')) AND (c <= ORD('9')) DO
        val := val * 10 + (c - ORD('0'));
        c := getchar()
    END;
    IF neg THEN n := -val ELSE n := val END;
    Done := TRUE
END ReadInt;

PROCEDURE ReadCard(VAR n: CARDINAL);
VAR
    c: INTEGER;
    val: CARDINAL;
BEGIN
    c := getchar();
    WHILE (c = ORD(' ')) OR (c = 9) OR (c = 10) OR (c = 13) DO
        c := getchar()
    END;
    IF (c = -1) OR (c < ORD('0')) OR (c > ORD('9')) THEN
        Done := FALSE; n := 0; RETURN
    END;
    val := 0;
    WHILE (c >= ORD('0')) AND (c <= ORD('9')) DO
        val := val * 10 + CARDINAL(c - ORD('0'));
        c := getchar()
    END;
    n := val;
    Done := TRUE
END ReadCard;

(* ── File operations (stub — rarely used in practice) ── *)

PROCEDURE OpenInput(ext: ARRAY OF CHAR);
BEGIN
    Done := FALSE  (* not implemented in native M2 stdlib *)
END OpenInput;

PROCEDURE OpenOutput(ext: ARRAY OF CHAR);
BEGIN
    Done := FALSE
END OpenOutput;

PROCEDURE CloseInput;
BEGIN
END CloseInput;

PROCEDURE CloseOutput;
BEGIN
END CloseOutput;

BEGIN
    Done := TRUE
END InOut.
