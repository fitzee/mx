IMPLEMENTATION MODULE SLongIO;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM CIO IMPORT putchar;
FROM CFmt IMPORT m2_fmt_long_e, m2_fmt_long_f, m2_fmt_long_g, m2_scan_longreal;

PROCEDURE WriteBuf(VAR buf: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
    i := 0;
    WHILE (i <= HIGH(buf)) AND (buf[i] # CHR(0)) DO
        putchar(ORD(buf[i]));
        INC(i)
    END
END WriteBuf;

PROCEDURE WriteFloat(r: LONGREAL; sigFigs: CARDINAL; w: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_long_e(ADR(buf), 64, r, INTEGER(sigFigs), INTEGER(w));
    WriteBuf(buf)
END WriteFloat;

PROCEDURE WriteFixed(r: LONGREAL; place: INTEGER; w: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_long_f(ADR(buf), 64, r, INTEGER(w), place);
    WriteBuf(buf)
END WriteFixed;

PROCEDURE WriteLongReal(r: LONGREAL; w: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_long_g(ADR(buf), 64, r, INTEGER(w));
    WriteBuf(buf)
END WriteLongReal;

PROCEDURE ReadLongReal(VAR r: LONGREAL);
VAR dummy: INTEGER;
BEGIN
    dummy := m2_scan_longreal(r)
END ReadLongReal;

END SLongIO.
