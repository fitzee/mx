IMPLEMENTATION MODULE STextIO;
FROM CIO IMPORT getchar, putchar;

PROCEDURE WriteChar(ch: CHAR);
BEGIN
    putchar(ORD(ch))
END WriteChar;

PROCEDURE ReadChar(VAR ch: CHAR);
VAR c: INTEGER;
BEGIN
    c := getchar();
    IF c = -1 THEN ch := CHR(0)
    ELSE ch := CHR(c)
    END
END ReadChar;

PROCEDURE WriteString(s: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
    i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # CHR(0)) DO
        putchar(ORD(s[i]));
        INC(i)
    END
END WriteString;

PROCEDURE ReadString(VAR s: ARRAY OF CHAR);
VAR c: INTEGER; i: CARDINAL;
BEGIN
    i := 0;
    c := getchar();
    WHILE (c # -1) AND (c # 10) AND (i <= HIGH(s)) DO
        s[i] := CHR(c);
        INC(i);
        c := getchar()
    END;
    IF i <= HIGH(s) THEN s[i] := CHR(0) END
END ReadString;

PROCEDURE WriteLn;
BEGIN
    putchar(10)
END WriteLn;

PROCEDURE SkipLine;
VAR c: INTEGER;
BEGIN
    c := getchar();
    WHILE (c # 10) AND (c # -1) DO
        c := getchar()
    END
END SkipLine;

PROCEDURE ReadToken(VAR s: ARRAY OF CHAR);
BEGIN
    ReadString(s)
END ReadToken;

END STextIO.
