IMPLEMENTATION MODULE Terminal;
FROM CIO IMPORT getchar, putchar;

VAR Done: BOOLEAN;

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

PROCEDURE Write(ch: CHAR);
BEGIN
    putchar(ORD(ch))
END Write;

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

BEGIN
    Done := TRUE
END Terminal.
