IMPLEMENTATION MODULE Strings;
(* Native M2 Strings — replaces C runtime m2_Strings_* functions.
   All operations NUL-terminate and truncate on overflow. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM CStr IMPORT strlen, memcpy, memmove, strcmp, strstr, toupper;

PROCEDURE Assign(s: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
VAR
    cap, slen: CARDINAL;
BEGIN
    cap := HIGH(dst) + 1;
    slen := strlen(ADR(s));
    IF slen >= cap THEN slen := cap - 1 END;
    memcpy(ADR(dst), ADR(s), slen);
    dst[slen] := CHR(0)
END Assign;

PROCEDURE Insert(sub: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR; pos: CARDINAL);
VAR
    cap, slen, dlen, newLen, tailDst, tailKeep, subCopy: CARDINAL;
BEGIN
    cap := HIGH(dst) + 1;
    slen := strlen(ADR(sub));
    dlen := strlen(ADR(dst));
    IF pos > dlen THEN pos := dlen END;
    newLen := dlen + slen;
    IF newLen >= cap THEN newLen := cap - 1 END;
    tailDst := pos + slen;
    IF tailDst < newLen THEN
        tailKeep := newLen - tailDst
    ELSE
        tailKeep := 0
    END;
    IF tailKeep > 0 THEN
        memmove(ADR(dst[tailDst]), ADR(dst[pos]), tailKeep)
    END;
    subCopy := slen;
    IF pos + subCopy > newLen THEN subCopy := newLen - pos END;
    IF subCopy > 0 THEN
        memcpy(ADR(dst[pos]), ADR(sub), subCopy)
    END;
    dst[newLen] := CHR(0)
END Insert;

PROCEDURE Delete(VAR s: ARRAY OF CHAR; pos: CARDINAL; len: CARDINAL);
VAR
    slen: CARDINAL;
BEGIN
    slen := strlen(ADR(s));
    IF pos >= slen THEN RETURN END;
    IF pos + len > slen THEN len := slen - pos END;
    memmove(ADR(s[pos]), ADR(s[pos + len]), slen - pos - len + 1)
END Delete;

PROCEDURE Pos(sub: ARRAY OF CHAR; s: ARRAY OF CHAR): CARDINAL;
VAR
    p, base: ADDRESS;
BEGIN
    base := ADR(s);
    p := strstr(base, ADR(sub));
    IF p = NIL THEN
        RETURN MAX(CARDINAL)
    ELSE
        RETURN CARDINAL(p - base)
    END
END Pos;

PROCEDURE Length(s: ARRAY OF CHAR): CARDINAL;
BEGIN
    RETURN strlen(ADR(s))
END Length;

PROCEDURE Copy(src: ARRAY OF CHAR; pos: CARDINAL; len: CARDINAL;
               VAR dst: ARRAY OF CHAR);
VAR
    cap, slen: CARDINAL;
BEGIN
    cap := HIGH(dst) + 1;
    slen := strlen(ADR(src));
    IF pos >= slen THEN
        dst[0] := CHR(0);
        RETURN
    END;
    IF pos + len > slen THEN len := slen - pos END;
    IF len >= cap THEN len := cap - 1 END;
    memcpy(ADR(dst), ADR(src[pos]), len);
    dst[len] := CHR(0)
END Copy;

PROCEDURE Concat(s1: ARRAY OF CHAR; s2: ARRAY OF CHAR;
                 VAR dst: ARRAY OF CHAR);
VAR
    cap, len1, len2, rem: CARDINAL;
BEGIN
    cap := HIGH(dst) + 1;
    len1 := strlen(ADR(s1));
    len2 := strlen(ADR(s2));
    IF len1 >= cap THEN len1 := cap - 1 END;
    memcpy(ADR(dst), ADR(s1), len1);
    rem := cap - 1 - len1;
    IF len2 > rem THEN len2 := rem END;
    memcpy(ADR(dst[len1]), ADR(s2), len2);
    dst[len1 + len2] := CHR(0)
END Concat;

PROCEDURE CompareStr(s1: ARRAY OF CHAR; s2: ARRAY OF CHAR): INTEGER;
BEGIN
    RETURN strcmp(ADR(s1), ADR(s2))
END CompareStr;

PROCEDURE CAPS(VAR s: ARRAY OF CHAR);
VAR
    i: CARDINAL;
BEGIN
    i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # CHR(0)) DO
        s[i] := CHR(toupper(ORD(s[i])));
        INC(i)
    END
END CAPS;

END Strings.
