IMPLEMENTATION MODULE BinaryIO;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM CIO IMPORT fopen, fclose, fgetc, fputc, fread, fwrite, fseek, ftell, feof;

CONST
    MaxFiles = 32;
    SeekSet = 0;
    SeekEnd = 2;

VAR
    Done: BOOLEAN;
    files: ARRAY [0..MaxFiles-1] OF ADDRESS;
    initialized: BOOLEAN;

PROCEDURE EnsureInit;
VAR i: CARDINAL;
BEGIN
    IF NOT initialized THEN
        FOR i := 0 TO MaxFiles - 1 DO
            files[i] := NIL
        END;
        initialized := TRUE
    END
END EnsureInit;

PROCEDURE AllocSlot(): INTEGER;
VAR i: CARDINAL;
BEGIN
    EnsureInit;
    FOR i := 0 TO MaxFiles - 1 DO
        IF files[i] = NIL THEN RETURN INTEGER(i) END
    END;
    RETURN -1
END AllocSlot;

PROCEDURE OpenRead(name: ARRAY OF CHAR; VAR fh: CARDINAL);
VAR slot: INTEGER; rb: ARRAY [0..2] OF CHAR;
BEGIN
    slot := AllocSlot();
    IF slot < 0 THEN
        Done := FALSE; fh := 0; RETURN
    END;
    rb[0] := 'r'; rb[1] := 'b'; rb[2] := CHR(0);
    files[slot] := fopen(ADR(name), ADR(rb));
    IF files[slot] # NIL THEN
        fh := CARDINAL(slot + 1);
        Done := TRUE
    ELSE
        fh := 0;
        Done := FALSE
    END
END OpenRead;

PROCEDURE OpenWrite(name: ARRAY OF CHAR; VAR fh: CARDINAL);
VAR slot: INTEGER; wb: ARRAY [0..2] OF CHAR;
BEGIN
    slot := AllocSlot();
    IF slot < 0 THEN
        Done := FALSE; fh := 0; RETURN
    END;
    wb[0] := 'w'; wb[1] := 'b'; wb[2] := CHR(0);
    files[slot] := fopen(ADR(name), ADR(wb));
    IF files[slot] # NIL THEN
        fh := CARDINAL(slot + 1);
        Done := TRUE
    ELSE
        fh := 0;
        Done := FALSE
    END
END OpenWrite;

PROCEDURE Close(fh: CARDINAL);
BEGIN
    EnsureInit;
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        fclose(files[fh-1]);
        files[fh-1] := NIL
    END
END Close;

PROCEDURE ReadByte(fh: CARDINAL; VAR b: CARDINAL);
VAR c: INTEGER;
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        c := fgetc(files[fh-1]);
        IF c = -1 THEN
            b := 0; Done := FALSE
        ELSE
            b := CARDINAL(c); Done := TRUE
        END
    ELSE
        b := 0; Done := FALSE
    END
END ReadByte;

PROCEDURE WriteByte(fh: CARDINAL; b: CARDINAL);
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        fputc(INTEGER(b), files[fh-1]);
        Done := TRUE
    ELSE
        Done := FALSE
    END
END WriteByte;

PROCEDURE ReadBytes(fh: CARDINAL; VAR buf: ARRAY OF CHAR; n: CARDINAL; VAR actual: CARDINAL);
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        actual := fread(ADR(buf), 1, n, files[fh-1]);
        Done := (actual > 0)
    ELSE
        actual := 0; Done := FALSE
    END
END ReadBytes;

PROCEDURE WriteBytes(fh: CARDINAL; buf: ARRAY OF CHAR; n: CARDINAL);
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        fwrite(ADR(buf), 1, n, files[fh-1]);
        Done := TRUE
    ELSE
        Done := FALSE
    END
END WriteBytes;

PROCEDURE FileSize(fh: CARDINAL; VAR size: CARDINAL);
VAR cur: INTEGER;
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        cur := ftell(files[fh-1]);
        fseek(files[fh-1], 0, SeekEnd);
        size := CARDINAL(ftell(files[fh-1]));
        fseek(files[fh-1], cur, SeekSet);
        Done := TRUE
    ELSE
        size := 0; Done := FALSE
    END
END FileSize;

PROCEDURE Seek(fh: CARDINAL; pos: CARDINAL);
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        fseek(files[fh-1], INTEGER(pos), SeekSet);
        Done := TRUE
    ELSE
        Done := FALSE
    END
END Seek;

PROCEDURE Tell(fh: CARDINAL; VAR pos: CARDINAL);
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        pos := CARDINAL(ftell(files[fh-1]));
        Done := TRUE
    ELSE
        pos := 0; Done := FALSE
    END
END Tell;

PROCEDURE IsEOF(fh: CARDINAL): BOOLEAN;
BEGIN
    IF (fh >= 1) AND (fh <= MaxFiles) AND (files[fh-1] # NIL) THEN
        RETURN feof(files[fh-1]) # 0
    END;
    RETURN TRUE
END IsEOF;

BEGIN
    Done := TRUE;
    initialized := FALSE
END BinaryIO.
