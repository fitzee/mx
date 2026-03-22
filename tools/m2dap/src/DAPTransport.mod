IMPLEMENTATION MODULE DAPTransport;
FROM SYSTEM IMPORT ADR, ADDRESS;
FROM ProcessBridge IMPORT m2dap_read_stdin, m2dap_write_stdout;

CONST
  MaxHeader = 256;
  CR = CHR(13);
  LF = CHR(10);

VAR
  (* Intermediate read buffer for stdin — we may read more than
     one byte at a time for efficiency. *)
  readBuf: ARRAY [0..4095] OF CHAR;
  readPos: CARDINAL;
  readLen: CARDINAL;

PROCEDURE FillBuf(): BOOLEAN;
(* Refill readBuf from stdin. Returns FALSE on EOF/error. *)
VAR n: INTEGER;
BEGIN
  n := m2dap_read_stdin(ADR(readBuf), 4096);
  IF n <= 0 THEN
    RETURN FALSE
  END;
  readPos := 0;
  readLen := VAL(CARDINAL, n);
  RETURN TRUE
END FillBuf;

PROCEDURE GetByte(VAR ch: CHAR): BOOLEAN;
(* Read one byte from buffered stdin. *)
BEGIN
  IF readPos >= readLen THEN
    IF NOT FillBuf() THEN RETURN FALSE END
  END;
  ch := readBuf[readPos];
  INC(readPos);
  RETURN TRUE
END GetByte;

PROCEDURE ReadMessage(VAR buf: ARRAY OF CHAR;
                      VAR len: CARDINAL): BOOLEAN;
VAR
  hdr: ARRAY [0..MaxHeader-1] OF CHAR;
  hdrLen: CARDINAL;
  ch: CHAR;
  contentLen: CARDINAL;
  i: CARDINAL;
  foundHeader: BOOLEAN;
  crlfCount: CARDINAL;
BEGIN
  (* Read header lines until empty line (CRLFCRLF).
     We only care about Content-Length. *)
  contentLen := 0;
  foundHeader := FALSE;

  LOOP
    (* Read one header line *)
    hdrLen := 0;
    LOOP
      IF NOT GetByte(ch) THEN RETURN FALSE END;
      IF ch = LF THEN EXIT END;
      IF (ch # CR) AND (hdrLen < MaxHeader - 1) THEN
        hdr[hdrLen] := ch;
        INC(hdrLen)
      END
    END;
    hdr[hdrLen] := CHR(0);

    (* Empty line = end of headers *)
    IF hdrLen = 0 THEN EXIT END;

    (* Check for "Content-Length: " prefix *)
    IF hdrLen > 16 THEN
      IF (hdr[0] = 'C') AND (hdr[1] = 'o') AND (hdr[2] = 'n') AND
         (hdr[3] = 't') AND (hdr[4] = 'e') AND (hdr[5] = 'n') AND
         (hdr[6] = 't') AND (hdr[7] = '-') AND (hdr[8] = 'L') AND
         (hdr[9] = 'e') AND (hdr[10] = 'n') AND (hdr[11] = 'g') AND
         (hdr[12] = 't') AND (hdr[13] = 'h') AND (hdr[14] = ':') AND
         (hdr[15] = ' ') THEN
        (* Parse number *)
        contentLen := 0;
        i := 16;
        WHILE (i < hdrLen) AND (hdr[i] >= '0') AND (hdr[i] <= '9') DO
          contentLen := contentLen * 10 +
                        (VAL(CARDINAL, ORD(hdr[i])) - VAL(CARDINAL, ORD('0')));
          INC(i)
        END;
        foundHeader := TRUE
      END
    END
  END;

  IF NOT foundHeader THEN RETURN FALSE END;
  IF contentLen = 0 THEN RETURN FALSE END;
  IF contentLen > HIGH(buf) THEN RETURN FALSE END;

  (* Read exactly contentLen bytes of body *)
  i := 0;
  WHILE i < contentLen DO
    IF NOT GetByte(ch) THEN RETURN FALSE END;
    buf[i] := ch;
    INC(i)
  END;
  IF contentLen <= HIGH(buf) THEN
    buf[contentLen] := CHR(0)
  END;
  len := contentLen;
  RETURN TRUE
END ReadMessage;

PROCEDURE WriteMessage(VAR buf: ARRAY OF CHAR; len: CARDINAL);
VAR
  hdr: ARRAY [0..63] OF CHAR;
  hdrLen: CARDINAL;
  digits: ARRAY [0..15] OF CHAR;
  nDigits: CARDINAL;
  val: CARDINAL;
  i: CARDINAL;
  n: INTEGER;
BEGIN
  hdr := "Content-Length: ";
  hdrLen := 16;

  (* Convert len to decimal digits *)
  IF len = 0 THEN
    hdr[hdrLen] := '0';
    INC(hdrLen)
  ELSE
    nDigits := 0;
    val := len;
    WHILE val > 0 DO
      digits[nDigits] := CHR(VAL(CARDINAL, ORD('0')) + (val MOD 10));
      INC(nDigits);
      val := val DIV 10
    END;
    (* Reverse digits into hdr *)
    i := nDigits;
    WHILE i > 0 DO
      DEC(i);
      hdr[hdrLen] := digits[i];
      INC(hdrLen)
    END
  END;

  hdr[hdrLen] := CR; INC(hdrLen);
  hdr[hdrLen] := LF; INC(hdrLen);
  hdr[hdrLen] := CR; INC(hdrLen);
  hdr[hdrLen] := LF; INC(hdrLen);

  (* Write header then body *)
  n := m2dap_write_stdout(ADR(hdr), VAL(INTEGER, hdrLen));
  n := m2dap_write_stdout(ADR(buf), VAL(INTEGER, len))
END WriteMessage;

BEGIN
  readPos := 0;
  readLen := 0
END DAPTransport.
