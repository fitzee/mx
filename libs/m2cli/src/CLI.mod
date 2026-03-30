IMPLEMENTATION MODULE CLI;

FROM Strings IMPORT Assign, Length, CompareStr, Concat;
FROM InOut IMPORT WriteString, WriteLn;

CONST
  MaxSpecs = 32;

TYPE
  SpecKind = (Flag, Option);

VAR
  specShort: ARRAY [0..MaxSpecs-1] OF ARRAY [0..7] OF CHAR;
  specLong: ARRAY [0..MaxSpecs-1] OF ARRAY [0..31] OF CHAR;
  specDesc: ARRAY [0..MaxSpecs-1] OF ARRAY [0..63] OF CHAR;
  specKind: ARRAY [0..MaxSpecs-1] OF SpecKind;
  specPresent: ARRAY [0..MaxSpecs-1] OF INTEGER;
  specValue: ARRAY [0..MaxSpecs-1] OF ARRAY [0..255] OF CHAR;
  specCount: INTEGER;

PROCEDURE FindByShort(s: ARRAY OF CHAR): INTEGER;
VAR j: INTEGER;
    cmp: ARRAY [0..7] OF CHAR;
BEGIN
  (* s might be "-x", strip the leading '-' *)
  IF (Length(s) >= 2) AND (s[0] = '-') AND (s[1] # '-') THEN
    cmp[0] := s[1]; cmp[1] := 0C
  ELSE
    Assign(s, cmp)
  END;
  FOR j := 0 TO specCount - 1 DO
    IF CompareStr(cmp, specShort[j]) = 0 THEN RETURN j END
  END;
  RETURN -1
END FindByShort;

PROCEDURE FindByLong(s: ARRAY OF CHAR): INTEGER;
VAR j: INTEGER;
    cmp: ARRAY [0..31] OF CHAR;
    p: INTEGER;
BEGIN
  (* s might be "--long", strip the leading '--' *)
  IF (Length(s) >= 3) AND (s[0] = '-') AND (s[1] = '-') THEN
    p := 0;
    WHILE (p + 2 < Length(s)) DO
      cmp[p] := s[p + 2]; INC(p)
    END;
    cmp[p] := 0C
  ELSE
    Assign(s, cmp)
  END;
  FOR j := 0 TO specCount - 1 DO
    IF CompareStr(cmp, specLong[j]) = 0 THEN RETURN j END
  END;
  RETURN -1
END FindByLong;

PROCEDURE AddFlag(short: ARRAY OF CHAR; long: ARRAY OF CHAR;
                  description: ARRAY OF CHAR);
BEGIN
  IF specCount >= MaxSpecs THEN RETURN END;
  Assign(short, specShort[specCount]);
  Assign(long, specLong[specCount]);
  Assign(description, specDesc[specCount]);
  specKind[specCount] := Flag;
  specPresent[specCount] := 0;
  specValue[specCount][0] := 0C;
  INC(specCount)
END AddFlag;

PROCEDURE AddOption(short: ARRAY OF CHAR; long: ARRAY OF CHAR;
                    description: ARRAY OF CHAR);
BEGIN
  IF specCount >= MaxSpecs THEN RETURN END;
  Assign(short, specShort[specCount]);
  Assign(long, specLong[specCount]);
  Assign(description, specDesc[specCount]);
  specKind[specCount] := Option;
  specPresent[specCount] := 0;
  specValue[specCount][0] := 0C;
  INC(specCount)
END AddOption;

PROCEDURE Parse(ac: CARDINAL; getArg: GetArgProc);
VAR
  j, idx: INTEGER;
  a: ARRAY [0..255] OF CHAR;
BEGIN
  j := 1; (* skip argv[0] = program name *)
  WHILE j < VAL(INTEGER, ac) DO
    getArg(VAL(CARDINAL, j), a);
    IF (Length(a) >= 2) AND (a[0] = '-') AND (a[1] = '-') THEN
      (* Long form *)
      idx := FindByLong(a);
      IF idx >= 0 THEN
        specPresent[idx] := 1;
        IF specKind[idx] = Option THEN
          INC(j);
          IF j < VAL(INTEGER, ac) THEN
            getArg(VAL(CARDINAL, j), specValue[idx])
          END
        END
      END
    ELSIF (Length(a) >= 2) AND (a[0] = '-') THEN
      (* Short form *)
      idx := FindByShort(a);
      IF idx >= 0 THEN
        specPresent[idx] := 1;
        IF specKind[idx] = Option THEN
          INC(j);
          IF j < VAL(INTEGER, ac) THEN
            getArg(VAL(CARDINAL, j), specValue[idx])
          END
        END
      END
    END;
    INC(j)
  END
END Parse;

PROCEDURE HasFlag(long: ARRAY OF CHAR): INTEGER;
VAR j: INTEGER;
BEGIN
  FOR j := 0 TO specCount - 1 DO
    IF CompareStr(long, specLong[j]) = 0 THEN
      RETURN specPresent[j]
    END
  END;
  RETURN 0
END HasFlag;

PROCEDURE GetOption(long: ARRAY OF CHAR; VAR buf: ARRAY OF CHAR): INTEGER;
VAR j: INTEGER;
BEGIN
  FOR j := 0 TO specCount - 1 DO
    IF CompareStr(long, specLong[j]) = 0 THEN
      IF specPresent[j] = 1 THEN
        Assign(specValue[j], buf);
        RETURN 1
      ELSE
        buf[0] := 0C;
        RETURN 0
      END
    END
  END;
  buf[0] := 0C;
  RETURN 0
END GetOption;

PROCEDURE PrintHelp;
VAR j: INTEGER;
    tmp: ARRAY [0..127] OF CHAR;
BEGIN
  WriteString("Options:"); WriteLn;
  FOR j := 0 TO specCount - 1 DO
    WriteString("  -"); WriteString(specShort[j]);
    WriteString(", --"); WriteString(specLong[j]);
    IF specKind[j] = Option THEN
      WriteString(" <value>")
    END;
    WriteString("  "); WriteString(specDesc[j]);
    WriteLn
  END
END PrintHelp;

BEGIN
  specCount := 0
END CLI.
