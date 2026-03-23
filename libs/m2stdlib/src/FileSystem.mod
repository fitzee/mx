IMPLEMENTATION MODULE FileSystem;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM CIO IMPORT fopen, fclose, fgetc, fputc;

TYPE File = ADDRESS;

VAR Done: BOOLEAN;

PROCEDURE Lookup(VAR f: File; name: ARRAY OF CHAR; newFile: BOOLEAN);
VAR mode: ARRAY [0..2] OF CHAR;
BEGIN
    IF newFile THEN
        mode[0] := 'w'; mode[1] := '+'; mode[2] := CHR(0)
    ELSE
        mode[0] := 'r'; mode[1] := '+'; mode[2] := CHR(0)
    END;
    f := fopen(ADR(name), ADR(mode));
    IF (f = NIL) AND NOT newFile THEN
        mode[0] := 'r'; mode[1] := CHR(0);
        f := fopen(ADR(name), ADR(mode))
    END;
    Done := (f # NIL)
END Lookup;

PROCEDURE Close(VAR f: File);
BEGIN
    IF f # NIL THEN
        fclose(f);
        f := NIL
    END
END Close;

PROCEDURE ReadChar(VAR f: File; VAR ch: CHAR);
VAR c: INTEGER;
BEGIN
    c := fgetc(f);
    IF c = -1 THEN
        ch := CHR(0);
        Done := FALSE
    ELSE
        ch := CHR(c);
        Done := TRUE
    END
END ReadChar;

PROCEDURE WriteChar(VAR f: File; ch: CHAR);
BEGIN
    fputc(ORD(ch), f)
END WriteChar;

BEGIN
    Done := TRUE
END FileSystem.
