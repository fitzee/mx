IMPLEMENTATION MODULE SRealIO;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM CIO IMPORT putchar;
FROM CFmt IMPORT m2_fmt_real_e, m2_fmt_real_f, m2_fmt_real_g, m2_scan_real;

PROCEDURE WriteBuf(VAR buf: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
    i := 0;
    WHILE (i <= HIGH(buf)) AND (buf[i] # CHR(0)) DO
        putchar(ORD(buf[i]));
        INC(i)
    END
END WriteBuf;

PROCEDURE WriteFloat(r: REAL; sigFigs: CARDINAL; w: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_real_e(ADR(buf), 64, r, INTEGER(sigFigs), INTEGER(w));
    WriteBuf(buf)
END WriteFloat;

PROCEDURE WriteFixed(r: REAL; place: INTEGER; w: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_real_f(ADR(buf), 64, r, INTEGER(w), place);
    WriteBuf(buf)
END WriteFixed;

PROCEDURE WriteReal(r: REAL; w: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_real_g(ADR(buf), 64, r, INTEGER(w));
    WriteBuf(buf)
END WriteReal;

PROCEDURE ReadReal(VAR r: REAL);
VAR dummy: INTEGER;
BEGIN
    dummy := m2_scan_real(r)
END ReadReal;

END SRealIO.
