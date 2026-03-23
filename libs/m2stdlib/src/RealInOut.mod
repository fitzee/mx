IMPLEMENTATION MODULE RealInOut;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM CIO IMPORT putchar;
FROM CFmt IMPORT m2_fmt_real_g, m2_fmt_real_f, m2_fmt_real_oct, m2_scan_real;

VAR Done: BOOLEAN;

PROCEDURE WriteBuf(VAR buf: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
    i := 0;
    WHILE (i <= HIGH(buf)) AND (buf[i] # CHR(0)) DO
        putchar(ORD(buf[i]));
        INC(i)
    END
END WriteBuf;

PROCEDURE ReadReal(VAR r: REAL);
BEGIN
    Done := (m2_scan_real(r) = 1)
END ReadReal;

PROCEDURE WriteReal(r: REAL; w: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_real_g(ADR(buf), 64, r, INTEGER(w));
    WriteBuf(buf)
END WriteReal;

PROCEDURE WriteFixPt(r: REAL; w: CARDINAL; d: CARDINAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_real_f(ADR(buf), 64, r, INTEGER(w), INTEGER(d));
    WriteBuf(buf)
END WriteFixPt;

PROCEDURE WriteRealOct(r: REAL);
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
    m2_fmt_real_oct(ADR(buf), 64, r);
    WriteBuf(buf)
END WriteRealOct;

BEGIN
    Done := TRUE
END RealInOut.
