IMPLEMENTATION MODULE M2Format;
FROM Strings IMPORT Assign, Length, CompareStr, Concat;

PROCEDURE StrEq(VAR a, b: ARRAY OF CHAR): BOOLEAN;
BEGIN
  RETURN CompareStr(a, b) = 0
END StrEq;

PROCEDURE StartsWith(VAR s, prefix: ARRAY OF CHAR): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(prefix)) AND (prefix[i] # CHR(0)) DO
    IF (i > HIGH(s)) OR (s[i] # prefix[i]) THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END StartsWith;

PROCEDURE EndsWith(VAR s, suffix: ARRAY OF CHAR): BOOLEAN;
VAR sLen, sufLen, i: CARDINAL;
BEGIN
  sLen := Length(s);
  sufLen := Length(suffix);
  IF sufLen > sLen THEN RETURN FALSE END;
  i := 0;
  WHILE i < sufLen DO
    IF s[sLen - sufLen + i] # suffix[i] THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END EndsWith;

PROCEDURE IsNullPtr(VAR val: ARRAY OF CHAR): BOOLEAN;
VAR
  null: ARRAY [0..7] OF CHAR;
  zero16: ARRAY [0..19] OF CHAR;
  zero8: ARRAY [0..11] OF CHAR;
  zero3: ARRAY [0..3] OF CHAR;
  zero1: ARRAY [0..1] OF CHAR;
BEGIN
  null := "NULL";
  zero16 := "0x0000000000000000";
  zero8 := "0x00000000";
  zero3 := "0x0";
  zero1 := "0";
  RETURN StrEq(val, null) OR StrEq(val, zero16) OR
         StrEq(val, zero8) OR StrEq(val, zero3) OR
         StrEq(val, zero1)
END IsNullPtr;

PROCEDURE IntToDecStr(val: INTEGER; VAR out: ARRAY OF CHAR);
VAR
  digits: ARRAY [0..15] OF CHAR;
  nd, j, k: CARDINAL;
  v: CARDINAL;
  neg: BOOLEAN;
BEGIN
  k := 0;
  neg := val < 0;
  IF neg THEN
    out[k] := '-'; INC(k);
    v := VAL(CARDINAL, -val)
  ELSIF val = 0 THEN
    out[0] := '0'; out[1] := CHR(0);
    RETURN
  ELSE
    v := VAL(CARDINAL, val)
  END;
  nd := 0;
  WHILE v > 0 DO
    digits[nd] := CHR(ORD('0') + (v MOD 10));
    INC(nd);
    v := v DIV 10
  END;
  j := nd;
  WHILE j > 0 DO
    DEC(j);
    IF k <= HIGH(out) THEN out[k] := digits[j]; INC(k) END
  END;
  IF k <= HIGH(out) THEN out[k] := CHR(0) END
END IntToDecStr;

(* ── FormatValue ─────────────────────────────────── *)

PROCEDURE FormatValue(VAR typeName: ARRAY OF CHAR;
                      VAR rawValue: ARRAY OF CHAR;
                      VAR out: ARRAY OF CHAR): BOOLEAN;
VAR
  s: ARRAY [0..31] OF CHAR;
  val: INTEGER;
  i, k: CARDINAL;
  digits: ARRAY [0..7] OF CHAR;
  nd, j: CARDINAL;
  v: CARDINAL;
  ch: CHAR;
BEGIN
  (* ── BOOLEAN: "unsigned int" with value 0 or 1 ── *)
  (* We check the DWARF name which FormatType maps, but lldb reports
     C types. The caller should pass the lldb type. We handle both. *)
  s := "unsigned int";
  IF StrEq(typeName, s) THEN
    s := "0";
    IF StrEq(rawValue, s) THEN
      Assign("FALSE", out)
    ELSE
      Assign("TRUE", out)
    END;
    RETURN TRUE
  END;
  s := "BOOLEAN";
  IF StrEq(typeName, s) THEN
    s := "0";
    IF StrEq(rawValue, s) THEN
      Assign("FALSE", out)
    ELSE
      Assign("TRUE", out)
    END;
    RETURN TRUE
  END;

  (* ── CHAR: "unsigned char" — lldb already shows 'A' for printable ── *)
  s := "unsigned char";
  IF StrEq(typeName, s) THEN
    (* lldb shows character literal like 'A' or numeric for non-printable.
       If already a char literal, pass through. If numeric, convert. *)
    IF (rawValue[0] = "'") THEN
      Assign(rawValue, out);
      RETURN TRUE
    END;
    IF (rawValue[0] >= '0') AND (rawValue[0] <= '9') THEN
      val := 0; i := 0;
      WHILE (i <= HIGH(rawValue)) AND (rawValue[i] >= '0') AND
            (rawValue[i] <= '9') DO
        val := val * 10 + (ORD(rawValue[i]) - ORD('0'));
        INC(i)
      END;
      IF (val >= 32) AND (val <= 126) THEN
        out[0] := "'"; out[1] := CHR(val); out[2] := "'"; out[3] := CHR(0)
      ELSE
        out[0] := 'C'; out[1] := 'H'; out[2] := 'R'; out[3] := '(';
        k := 4;
        IF val = 0 THEN
          out[k] := '0'; INC(k)
        ELSE
          nd := 0; v := VAL(CARDINAL, val);
          WHILE v > 0 DO
            digits[nd] := CHR(ORD('0') + (v MOD 10));
            INC(nd); v := v DIV 10
          END;
          j := nd;
          WHILE j > 0 DO
            DEC(j); out[k] := digits[j]; INC(k)
          END
        END;
        out[k] := ')'; INC(k); out[k] := CHR(0)
      END;
      RETURN TRUE
    END;
    (* lldb shows 'A' format — pass through *)
    Assign(rawValue, out);
    RETURN FALSE
  END;
  s := "CHAR";
  IF StrEq(typeName, s) THEN
    (* Same logic for DWARF CHAR name *)
    IF (rawValue[0] >= '0') AND (rawValue[0] <= '9') THEN
      val := 0; i := 0;
      WHILE (i <= HIGH(rawValue)) AND (rawValue[i] >= '0') AND
            (rawValue[i] <= '9') DO
        val := val * 10 + (ORD(rawValue[i]) - ORD('0'));
        INC(i)
      END;
      IF (val >= 32) AND (val <= 126) THEN
        out[0] := "'"; out[1] := CHR(val); out[2] := "'"; out[3] := CHR(0)
      ELSE
        out[0] := 'C'; out[1] := 'H'; out[2] := 'R'; out[3] := '(';
        k := 4;
        IF val = 0 THEN
          out[k] := '0'; INC(k)
        ELSE
          nd := 0; v := VAL(CARDINAL, val);
          WHILE v > 0 DO
            digits[nd] := CHR(ORD('0') + (v MOD 10));
            INC(nd); v := v DIV 10
          END;
          j := nd;
          WHILE j > 0 DO
            DEC(j); out[k] := digits[j]; INC(k)
          END
        END;
        out[k] := ')'; INC(k); out[k] := CHR(0)
      END;
      RETURN TRUE
    END;
    Assign(rawValue, out);
    RETURN FALSE
  END;

  (* ── Pointer/ADDRESS types: NULL/0x0 → NIL ── *)
  s := "ADDRESS";
  IF StrEq(typeName, s) THEN
    IF IsNullPtr(rawValue) THEN
      Assign("NIL", out); RETURN TRUE
    END;
    Assign(rawValue, out); RETURN FALSE
  END;
  (* lldb reports "Type *" for POINTER TO Type, "unsigned char *" for ADDRESS *)
  s := "*";
  IF EndsWith(typeName, s) THEN
    IF IsNullPtr(rawValue) THEN
      Assign("NIL", out); RETURN TRUE
    END;
    Assign(rawValue, out); RETURN FALSE
  END;

  (* ── REAL/float: pass through (lldb already formats) ── *)
  s := "float";
  IF StrEq(typeName, s) THEN
    Assign(rawValue, out);
    RETURN FALSE
  END;

  (* ── Record type: multi-line { field = val } — pass through ── *)
  (* DAPServer handles structured types separately *)

  (* ── No special formatting — pass through ── *)
  Assign(rawValue, out);
  RETURN FALSE
END FormatValue;

(* ── FormatType ──────────────────────────────────── *)

PROCEDURE FormatType(VAR typeName: ARRAY OF CHAR;
                     VAR out: ARRAY OF CHAR);
VAR
  s: ARRAY [0..31] OF CHAR;
  star: ARRAY [0..1] OF CHAR;
  len: CARDINAL;
BEGIN
  (* Map C type names back to M2 *)
  s := "int";
  IF StrEq(typeName, s) THEN Assign("INTEGER", out); RETURN END;
  s := "unsigned int";
  IF StrEq(typeName, s) THEN Assign("CARDINAL", out); RETURN END;
  s := "unsigned char";
  IF StrEq(typeName, s) THEN Assign("CHAR", out); RETURN END;
  s := "float";
  IF StrEq(typeName, s) THEN Assign("REAL", out); RETURN END;
  s := "double";
  IF StrEq(typeName, s) THEN Assign("LONGREAL", out); RETURN END;
  s := "long";
  IF StrEq(typeName, s) THEN Assign("LONGINT", out); RETURN END;
  s := "unsigned long";
  IF StrEq(typeName, s) THEN Assign("LONGCARD", out); RETURN END;
  s := "long long";
  IF StrEq(typeName, s) THEN Assign("LONGINT", out); RETURN END;
  s := "unsigned long long";
  IF StrEq(typeName, s) THEN Assign("LONGCARD", out); RETURN END;

  (* "unsigned char *" → ADDRESS *)
  s := "unsigned char *";
  IF StrEq(typeName, s) THEN Assign("ADDRESS", out); RETURN END;

  (* "Type *" → POINTER TO Type (keep struct name) *)
  star := "*";
  IF EndsWith(typeName, star) THEN
    len := Length(typeName);
    IF len >= 3 THEN
      (* Strip " *" suffix, prepend POINTER TO *)
      out[0] := CHR(0);
      Assign("POINTER TO ", out);
      (* Append the base type name (without trailing " *") *)
      s[0] := CHR(0);
      len := len - 2;  (* skip " *" *)
      IF len <= HIGH(s) THEN
        CopyN(typeName, s, len);
        Concat(out, s, out)
      END
    END;
    RETURN
  END;

  (* "int[N]" → ARRAY [...] OF INTEGER *)
  s := "int[";
  IF StartsWith(typeName, s) THEN
    Assign("ARRAY OF INTEGER", out); RETURN
  END;

  (* Named struct types — pass through as-is (already M2 names like "Point") *)
  Assign(typeName, out)
END FormatType;

(* ── Helper: copy N chars ── *)
PROCEDURE CopyN(VAR src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR; n: CARDINAL);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i < n) AND (i <= HIGH(src)) AND (i <= HIGH(dst)) DO
    dst[i] := src[i]; INC(i)
  END;
  IF i <= HIGH(dst) THEN dst[i] := CHR(0) END
END CopyN;

(* ── Demangle ────────────────────────────────────── *)

PROCEDURE Demangle(VAR src: ARRAY OF CHAR;
                   VAR out: ARRAY OF CHAR);
VAR
  i, j: CARDINAL;
  foundUnderscore: BOOLEAN;
BEGIN
  (* Replace first '_' with '.' — Module_Proc → Module.Proc *)
  i := 0;
  j := 0;
  foundUnderscore := FALSE;
  WHILE (i <= HIGH(src)) AND (src[i] # CHR(0)) AND
        (j < HIGH(out)) DO
    IF (src[i] = '_') AND (NOT foundUnderscore) THEN
      out[j] := '.';
      foundUnderscore := TRUE
    ELSE
      out[j] := src[i]
    END;
    INC(i); INC(j)
  END;
  out[j] := CHR(0)
END Demangle;

BEGIN
END M2Format.
