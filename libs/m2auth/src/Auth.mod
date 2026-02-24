IMPLEMENTATION MODULE Auth;

  (* Authentication & authorization: keyring, JWT HS256, Ed25519,
     policy, replay cache.  Uses auth_bridge.c (OpenSSL) for crypto. *)

  FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
  FROM Storage IMPORT ALLOCATE, DEALLOCATE;
  FROM Strings IMPORT Assign, Length, CompareStr;
  FROM AuthBridge IMPORT m2_auth_init,
                          m2_auth_get_unix_time,
                          m2_auth_b64url_encode,
                          m2_auth_b64url_decode,
                          m2_auth_hmac_sha256,
                          m2_auth_ct_compare,
                          m2_auth_has_ed25519,
                          m2_auth_ed25519_keygen,
                          m2_auth_ed25519_sign,
                          m2_auth_ed25519_verify;

  CONST
    MaxPayloadLen  = 1024;
    HmacLen        = 32;
    Ed25519SigLen  = 64;
    Ed25519PubLen  = 32;
    Ed25519PrivLen = 32;  (* raw seed *)
    MaxRawTokenLen = 1536;
    MaxPolicyRules = 32;

  TYPE
    KeyKind = (KkHS256, KkEd25519);

    KeyEntry = RECORD
      kid:    KeyId;
      kind:   KeyKind;
      sym:    SymKey;
      pub:    PublicKey;
      active: BOOLEAN;
    END;

    KeyringRec = RECORD
      keys:  ARRAY [0..MaxKeys-1] OF KeyEntry;
      count: CARDINAL;
    END;

    VerifierRec = RECORD
      kr:        Keyring;
      clockSkew: CARDINAL;
      aud:       ARRAY [0..MaxAudLen] OF CHAR;
      audSet:    BOOLEAN;
      iss:       ARRAY [0..MaxIssLen] OF CHAR;
      issSet:    BOOLEAN;
    END;

    ReplayEntry = RECORD
      jti:     ARRAY [0..MaxJtiLen] OF CHAR;
      expUnix: LONGINT;
      used:    BOOLEAN;
    END;

    ReplayCacheRec = RECORD
      entries: ARRAY [0..MaxReplay-1] OF ReplayEntry;
      count:   CARDINAL;
      next:    CARDINAL;
    END;

    PolicyRuleKind = (PrScope, PrClaim);

    PolicyRule = RECORD
      kind:  PolicyRuleKind;
      scope: ARRAY [0..MaxScopeLen] OF CHAR;
      key:   ARRAY [0..MaxClaimKey] OF CHAR;
      value: ARRAY [0..MaxClaimVal] OF CHAR;
    END;

    PolicyRec = RECORD
      rules: ARRAY [0..MaxPolicyRules-1] OF PolicyRule;
      count: CARDINAL;
    END;

    (* Working buffers for token operations *)
    RawBuf  = ARRAY [0..MaxRawTokenLen-1] OF CHAR;
    DataBuf = ARRAY [0..MaxPayloadLen-1] OF CHAR;

  (* ── String helpers ──────────────────────────────── *)

  PROCEDURE StrLen(VAR s: ARRAY OF CHAR): CARDINAL;
  VAR i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
    RETURN i
  END StrLen;

  PROCEDURE StrCopy(VAR src: ARRAY OF CHAR;
                    VAR dst: ARRAY OF CHAR);
  VAR i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(src)) AND (i <= HIGH(dst)) AND (src[i] # 0C) DO
      dst[i] := src[i]; INC(i)
    END;
    IF i <= HIGH(dst) THEN dst[i] := 0C END
  END StrCopy;

  PROCEDURE StrEqual(VAR a, b: ARRAY OF CHAR): BOOLEAN;
  VAR i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(a)) AND (i <= HIGH(b)) AND
          (a[i] # 0C) AND (a[i] = b[i]) DO
      INC(i)
    END;
    IF (i > HIGH(a)) OR (a[i] = 0C) THEN
      IF (i > HIGH(b)) OR (b[i] = 0C) THEN RETURN TRUE END
    END;
    RETURN FALSE
  END StrEqual;

  PROCEDURE ClearStr(VAR s: ARRAY OF CHAR);
  BEGIN
    s[0] := 0C
  END ClearStr;

  (* ── Status helpers ──────────────────────────────── *)

  PROCEDURE StatusToStr(s: Status; VAR out: ARRAY OF CHAR);
  BEGIN
    CASE s OF
      OK:           Assign("OK", out) |
      Invalid:      Assign("Invalid", out) |
      OutOfMemory:  Assign("OutOfMemory", out) |
      VerifyFailed: Assign("VerifyFailed", out) |
      Expired:      Assign("Expired", out) |
      NotYetValid:  Assign("NotYetValid", out) |
      BadSignature: Assign("BadSignature", out) |
      Unsupported:  Assign("Unsupported", out) |
      Denied:       Assign("Denied", out)
    END
  END StatusToStr;

  PROCEDURE InitPrincipal(VAR p: Principal);
  VAR i: CARDINAL;
  BEGIN
    ClearStr(p.subject);
    ClearStr(p.issuer);
    ClearStr(p.audience);
    p.scopeCount := 0;
    FOR i := 0 TO MaxScopes-1 DO
      ClearStr(p.scopes[i])
    END;
    p.claims.count := 0;
    p.expUnix := 0;
    p.nbfUnix := 0;
    p.iatUnix := 0;
    ClearStr(p.kid);
    ClearStr(p.jti)
  END InitPrincipal;

  (* ── Keyring ─────────────────────────────────────── *)

  PROCEDURE KeyringCreate(VAR kr: Keyring): Status;
  VAR p: POINTER TO KeyringRec;
  BEGIN
    ALLOCATE(p, TSIZE(KeyringRec));
    IF p = NIL THEN kr := NIL; RETURN OutOfMemory END;
    p^.count := 0;
    kr := p;
    RETURN OK
  END KeyringCreate;

  PROCEDURE KeyringDestroy(VAR kr: Keyring): Status;
  VAR p: POINTER TO KeyringRec;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    p := kr;
    DEALLOCATE(p, TSIZE(KeyringRec));
    kr := NIL;
    RETURN OK
  END KeyringDestroy;

  PROCEDURE FindKey(kr: Keyring; kid: ARRAY OF CHAR;
                    VAR idx: CARDINAL): BOOLEAN;
  VAR p: POINTER TO KeyringRec; i: CARDINAL;
  BEGIN
    p := kr;
    IF p^.count = 0 THEN RETURN FALSE END;
    FOR i := 0 TO p^.count - 1 DO
      IF StrEqual(p^.keys[i].kid, kid) THEN
        idx := i; RETURN TRUE
      END
    END;
    RETURN FALSE
  END FindKey;

  PROCEDURE KeyringAddEd25519Public(kr: Keyring;
                                     kid: ARRAY OF CHAR;
                                     VAR key: PublicKey): Status;
  VAR p: POINTER TO KeyringRec; idx, i: CARDINAL;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    IF FindKey(kr, kid, idx) THEN RETURN Invalid END;
    p := kr;
    IF p^.count >= MaxKeys THEN RETURN OutOfMemory END;
    idx := p^.count;
    StrCopy(kid, p^.keys[idx].kid);
    p^.keys[idx].kind := KkEd25519;
    FOR i := 0 TO 63 DO
      p^.keys[idx].pub[i] := key[i]
    END;
    p^.keys[idx].active := (p^.count = 0);
    INC(p^.count);
    RETURN OK
  END KeyringAddEd25519Public;

  PROCEDURE KeyringAddHS256(kr: Keyring;
                             kid: ARRAY OF CHAR;
                             VAR key: SymKey): Status;
  VAR p: POINTER TO KeyringRec; idx, i: CARDINAL;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    IF FindKey(kr, kid, idx) THEN RETURN Invalid END;
    p := kr;
    IF p^.count >= MaxKeys THEN RETURN OutOfMemory END;
    idx := p^.count;
    StrCopy(kid, p^.keys[idx].kid);
    p^.keys[idx].kind := KkHS256;
    FOR i := 0 TO 31 DO
      p^.keys[idx].sym[i] := key[i]
    END;
    p^.keys[idx].active := (p^.count = 0);
    INC(p^.count);
    RETURN OK
  END KeyringAddHS256;

  PROCEDURE KeyringRemove(kr: Keyring;
                           kid: ARRAY OF CHAR): Status;
  VAR p: POINTER TO KeyringRec; idx, i: CARDINAL;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    IF NOT FindKey(kr, kid, idx) THEN RETURN Invalid END;
    p := kr;
    (* Shift remaining keys down *)
    i := idx;
    WHILE i + 1 < p^.count DO
      p^.keys[i] := p^.keys[i+1]; INC(i)
    END;
    DEC(p^.count);
    RETURN OK
  END KeyringRemove;

  PROCEDURE KeyringSetActive(kr: Keyring;
                              kid: ARRAY OF CHAR): Status;
  VAR p: POINTER TO KeyringRec; idx, i: CARDINAL;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    IF NOT FindKey(kr, kid, idx) THEN RETURN Invalid END;
    p := kr;
    IF p^.count > 0 THEN
      FOR i := 0 TO p^.count - 1 DO
        p^.keys[i].active := FALSE
      END
    END;
    p^.keys[idx].active := TRUE;
    RETURN OK
  END KeyringSetActive;

  PROCEDURE KeyringList(kr: Keyring;
                         VAR kids: ARRAY OF KeyId;
                         VAR count: CARDINAL): Status;
  VAR p: POINTER TO KeyringRec; i: CARDINAL;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    p := kr;
    count := p^.count;
    IF p^.count > 0 THEN
      FOR i := 0 TO p^.count - 1 DO
        IF i <= HIGH(kids) THEN
          StrCopy(p^.keys[i].kid, kids[i])
        END
      END
    END;
    RETURN OK
  END KeyringList;

  PROCEDURE GetActiveKey(kr: Keyring; VAR idx: CARDINAL): BOOLEAN;
  VAR p: POINTER TO KeyringRec; i: CARDINAL;
  BEGIN
    p := kr;
    IF p^.count = 0 THEN RETURN FALSE END;
    FOR i := 0 TO p^.count - 1 DO
      IF p^.keys[i].active THEN idx := i; RETURN TRUE END
    END;
    RETURN FALSE
  END GetActiveKey;

  (* ── Verifier ────────────────────────────────────── *)

  PROCEDURE VerifierCreate(VAR v: Verifier;
                            kr: Keyring): Status;
  VAR p: POINTER TO VerifierRec;
  BEGIN
    IF kr = NIL THEN v := NIL; RETURN Invalid END;
    ALLOCATE(p, TSIZE(VerifierRec));
    IF p = NIL THEN v := NIL; RETURN OutOfMemory END;
    p^.kr := kr;
    p^.clockSkew := 60;
    p^.audSet := FALSE;
    p^.issSet := FALSE;
    ClearStr(p^.aud);
    ClearStr(p^.iss);
    v := p;
    RETURN OK
  END VerifierCreate;

  PROCEDURE VerifierDestroy(VAR v: Verifier): Status;
  VAR p: POINTER TO VerifierRec;
  BEGIN
    IF v = NIL THEN RETURN Invalid END;
    p := v;
    DEALLOCATE(p, TSIZE(VerifierRec));
    v := NIL;
    RETURN OK
  END VerifierDestroy;

  PROCEDURE VerifierSetClockSkewSeconds(v: Verifier;
                                         secs: CARDINAL): Status;
  VAR p: POINTER TO VerifierRec;
  BEGIN
    IF v = NIL THEN RETURN Invalid END;
    p := v;
    p^.clockSkew := secs;
    RETURN OK
  END VerifierSetClockSkewSeconds;

  PROCEDURE VerifierSetAudience(v: Verifier;
                                 aud: ARRAY OF CHAR): Status;
  VAR p: POINTER TO VerifierRec;
  BEGIN
    IF v = NIL THEN RETURN Invalid END;
    p := v;
    StrCopy(aud, p^.aud);
    p^.audSet := TRUE;
    RETURN OK
  END VerifierSetAudience;

  PROCEDURE VerifierSetIssuer(v: Verifier;
                               iss: ARRAY OF CHAR): Status;
  VAR p: POINTER TO VerifierRec;
  BEGIN
    IF v = NIL THEN RETURN Invalid END;
    p := v;
    StrCopy(iss, p^.iss);
    p^.issSet := TRUE;
    RETURN OK
  END VerifierSetIssuer;

  (* ── Internal JSON parser for JWT payloads ───────── *)

  (* Flat-object parser: recognizes known keys into Principal fields,
     extras become custom Claims.  No arrays/nesting. *)

  PROCEDURE SkipWhitespace(VAR buf: ARRAY OF CHAR;
                           VAR pos: CARDINAL; len: CARDINAL);
  BEGIN
    WHILE (pos < len) AND
          ((buf[pos] = ' ') OR (buf[pos] = 11C) OR
           (buf[pos] = 12C) OR (buf[pos] = 15C)) DO
      INC(pos)
    END
  END SkipWhitespace;

  PROCEDURE ParseJsonString(VAR buf: ARRAY OF CHAR;
                            VAR pos: CARDINAL; len: CARDINAL;
                            VAR out: ARRAY OF CHAR;
                            VAR outLen: CARDINAL): BOOLEAN;
  VAR ch: CHAR; oi: CARDINAL;
  BEGIN
    outLen := 0;
    IF (pos >= len) OR (buf[pos] # '"') THEN RETURN FALSE END;
    INC(pos);
    oi := 0;
    WHILE (pos < len) AND (buf[pos] # '"') DO
      ch := buf[pos];
      IF (ch = '\') AND (pos + 1 < len) THEN
        INC(pos);
        ch := buf[pos];
        IF ch = 'n' THEN ch := 12C
        ELSIF ch = 't' THEN ch := 11C
        ELSIF ch = '"' THEN ch := '"'
        ELSIF ch = '\' THEN ch := '\'
        END
      END;
      IF oi <= HIGH(out) THEN out[oi] := ch; INC(oi) END;
      INC(pos)
    END;
    IF (pos < len) AND (buf[pos] = '"') THEN INC(pos) END;
    IF oi <= HIGH(out) THEN out[oi] := 0C END;
    outLen := oi;
    RETURN TRUE
  END ParseJsonString;

  PROCEDURE ParseJsonInteger(VAR buf: ARRAY OF CHAR;
                             VAR pos: CARDINAL; len: CARDINAL;
                             VAR val: LONGINT): BOOLEAN;
  VAR neg: BOOLEAN; digit: CARDINAL;
  BEGIN
    val := 0;
    neg := FALSE;
    IF (pos < len) AND (buf[pos] = '-') THEN
      neg := TRUE; INC(pos)
    END;
    IF (pos >= len) OR (buf[pos] < '0') OR (buf[pos] > '9') THEN
      RETURN FALSE
    END;
    WHILE (pos < len) AND (buf[pos] >= '0') AND (buf[pos] <= '9') DO
      digit := ORD(buf[pos]) - ORD('0');
      val := val * 10 + LONGINT(digit);
      INC(pos)
    END;
    IF neg THEN val := -val END;
    RETURN TRUE
  END ParseJsonInteger;

  PROCEDURE ParseJsonBool(VAR buf: ARRAY OF CHAR;
                          VAR pos: CARDINAL; len: CARDINAL;
                          VAR val: BOOLEAN): BOOLEAN;
  BEGIN
    IF (pos + 3 < len) AND (buf[pos] = 't') AND
       (buf[pos+1] = 'r') AND (buf[pos+2] = 'u') AND
       (buf[pos+3] = 'e') THEN
      val := TRUE; INC(pos, 4); RETURN TRUE
    END;
    IF (pos + 4 < len) AND (buf[pos] = 'f') AND
       (buf[pos+1] = 'a') AND (buf[pos+2] = 'l') AND
       (buf[pos+3] = 's') AND (buf[pos+4] = 'e') THEN
      val := FALSE; INC(pos, 5); RETURN TRUE
    END;
    RETURN FALSE
  END ParseJsonBool;

  PROCEDURE StrEqLit(VAR s: ARRAY OF CHAR; lit: ARRAY OF CHAR): BOOLEAN;
  VAR i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(s)) AND (i <= HIGH(lit)) AND
          (s[i] # 0C) AND (s[i] = lit[i]) DO
      INC(i)
    END;
    IF (i > HIGH(lit)) OR (lit[i] = 0C) THEN
      IF (i > HIGH(s)) OR (s[i] = 0C) THEN RETURN TRUE END
    END;
    RETURN FALSE
  END StrEqLit;

  PROCEDURE ParseScopeString(VAR scopeStr: ARRAY OF CHAR;
                              VAR p: Principal);
  (* Split space-separated scopes into p.scopes array *)
  VAR si, di, sc: CARDINAL;
  BEGIN
    p.scopeCount := 0;
    si := 0;
    sc := 0;
    WHILE (si <= HIGH(scopeStr)) AND (scopeStr[si] # 0C) AND
          (sc < MaxScopes) DO
      di := 0;
      WHILE (si <= HIGH(scopeStr)) AND (scopeStr[si] # 0C) AND
            (scopeStr[si] # ' ') AND (di < MaxScopeLen) DO
        p.scopes[sc][di] := scopeStr[si];
        INC(si); INC(di)
      END;
      IF di <= MaxScopeLen THEN p.scopes[sc][di] := 0C END;
      IF di > 0 THEN INC(sc) END;
      WHILE (si <= HIGH(scopeStr)) AND (scopeStr[si] = ' ') DO
        INC(si)
      END
    END;
    p.scopeCount := sc
  END ParseScopeString;

  PROCEDURE ParseJwtPayload(VAR json: ARRAY OF CHAR;
                            jsonLen: CARDINAL;
                            VAR p: Principal): Status;
  (* Parse flat JSON object into Principal.
     Known keys: sub, iss, aud, exp, nbf, iat, jti, scope, kid.
     Unknown keys go into claims. *)
  VAR
    pos: CARDINAL;
    keyBuf: ARRAY [0..MaxClaimKey] OF CHAR;
    valBuf: ARRAY [0..MaxClaimVal] OF CHAR;
    keyLen, valLen: CARDINAL;
    intVal: LONGINT;
    boolVal: BOOLEAN;
    ci: CARDINAL;
  BEGIN
    InitPrincipal(p);
    pos := 0;
    SkipWhitespace(json, pos, jsonLen);
    IF (pos >= jsonLen) OR (json[pos] # '{') THEN RETURN Invalid END;
    INC(pos);

    LOOP
      SkipWhitespace(json, pos, jsonLen);
      IF (pos >= jsonLen) OR (json[pos] = '}') THEN EXIT END;

      (* Parse key *)
      IF NOT ParseJsonString(json, pos, jsonLen, keyBuf, keyLen) THEN
        RETURN Invalid
      END;
      SkipWhitespace(json, pos, jsonLen);
      IF (pos >= jsonLen) OR (json[pos] # ':') THEN RETURN Invalid END;
      INC(pos);
      SkipWhitespace(json, pos, jsonLen);

      (* Parse value based on known keys *)
      IF StrEqLit(keyBuf, "sub") THEN
        IF NOT ParseJsonString(json, pos, jsonLen,
                               p.subject, valLen) THEN
          RETURN Invalid
        END
      ELSIF StrEqLit(keyBuf, "iss") THEN
        IF NOT ParseJsonString(json, pos, jsonLen,
                               p.issuer, valLen) THEN
          RETURN Invalid
        END
      ELSIF StrEqLit(keyBuf, "aud") THEN
        IF NOT ParseJsonString(json, pos, jsonLen,
                               p.audience, valLen) THEN
          RETURN Invalid
        END
      ELSIF StrEqLit(keyBuf, "kid") THEN
        IF NOT ParseJsonString(json, pos, jsonLen,
                               p.kid, valLen) THEN
          RETURN Invalid
        END
      ELSIF StrEqLit(keyBuf, "jti") THEN
        IF NOT ParseJsonString(json, pos, jsonLen,
                               p.jti, valLen) THEN
          RETURN Invalid
        END
      ELSIF StrEqLit(keyBuf, "scope") THEN
        IF NOT ParseJsonString(json, pos, jsonLen,
                               valBuf, valLen) THEN
          RETURN Invalid
        END;
        ParseScopeString(valBuf, p)
      ELSIF StrEqLit(keyBuf, "exp") THEN
        IF NOT ParseJsonInteger(json, pos, jsonLen, p.expUnix) THEN
          RETURN Invalid
        END
      ELSIF StrEqLit(keyBuf, "nbf") THEN
        IF NOT ParseJsonInteger(json, pos, jsonLen, p.nbfUnix) THEN
          RETURN Invalid
        END
      ELSIF StrEqLit(keyBuf, "iat") THEN
        IF NOT ParseJsonInteger(json, pos, jsonLen, p.iatUnix) THEN
          RETURN Invalid
        END
      ELSE
        (* Unknown key — try string, then int, then bool → claim *)
        ci := p.claims.count;
        IF ci < MaxClaims THEN
          StrCopy(keyBuf, p.claims.items[ci].key);
          IF ParseJsonString(json, pos, jsonLen, valBuf, valLen) THEN
            p.claims.items[ci].vtype := Str;
            StrCopy(valBuf, p.claims.items[ci].s);
            p.claims.items[ci].i := 0;
            p.claims.items[ci].b := FALSE;
            INC(p.claims.count)
          ELSIF ParseJsonInteger(json, pos, jsonLen, intVal) THEN
            p.claims.items[ci].vtype := Int;
            p.claims.items[ci].i := intVal;
            ClearStr(p.claims.items[ci].s);
            p.claims.items[ci].b := FALSE;
            INC(p.claims.count)
          ELSIF ParseJsonBool(json, pos, jsonLen, boolVal) THEN
            p.claims.items[ci].vtype := Bool;
            p.claims.items[ci].b := boolVal;
            ClearStr(p.claims.items[ci].s);
            p.claims.items[ci].i := 0;
            INC(p.claims.count)
          ELSE
            RETURN Invalid
          END
        ELSE
          (* Skip value — too many claims *)
          IF NOT ParseJsonString(json, pos, jsonLen,
                                 valBuf, valLen) THEN
            IF NOT ParseJsonInteger(json, pos, jsonLen, intVal) THEN
              IF NOT ParseJsonBool(json, pos, jsonLen, boolVal) THEN
                RETURN Invalid
              END
            END
          END
        END
      END;

      SkipWhitespace(json, pos, jsonLen);
      IF (pos < jsonLen) AND (json[pos] = ',') THEN INC(pos) END
    END;

    RETURN OK
  END ParseJwtPayload;

  (* ── Internal JSON serializer for JWT payloads ───── *)

  PROCEDURE AppendChar(VAR buf: ARRAY OF CHAR;
                       VAR pos: CARDINAL; ch: CHAR);
  BEGIN
    IF pos <= HIGH(buf) THEN buf[pos] := ch; INC(pos) END
  END AppendChar;

  PROCEDURE AppendStr(VAR buf: ARRAY OF CHAR;
                      VAR pos: CARDINAL;
                      VAR s: ARRAY OF CHAR);
  VAR i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
      AppendChar(buf, pos, s[i]); INC(i)
    END
  END AppendStr;

  PROCEDURE AppendLit(VAR buf: ARRAY OF CHAR;
                      VAR pos: CARDINAL;
                      s: ARRAY OF CHAR);
  VAR i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
      AppendChar(buf, pos, s[i]); INC(i)
    END
  END AppendLit;

  PROCEDURE AppendLongInt(VAR buf: ARRAY OF CHAR;
                          VAR pos: CARDINAL; val: LONGINT);
  VAR digits: ARRAY [0..19] OF CHAR; di, i: CARDINAL;
      neg: BOOLEAN; v: LONGINT;
  BEGIN
    IF val = 0 THEN
      AppendChar(buf, pos, '0'); RETURN
    END;
    neg := (val < 0);
    IF neg THEN v := -val ELSE v := val END;
    di := 0;
    WHILE v > 0 DO
      digits[di] := CHR(ORD('0') + INTEGER(v MOD 10));
      v := v DIV 10;
      INC(di)
    END;
    IF neg THEN AppendChar(buf, pos, '-') END;
    FOR i := di TO 1 BY -1 DO
      AppendChar(buf, pos, digits[i-1])
    END
  END AppendLongInt;

  PROCEDURE AppendJsonStr(VAR buf: ARRAY OF CHAR;
                          VAR pos: CARDINAL;
                          VAR s: ARRAY OF CHAR);
  VAR i: CARDINAL;
  BEGIN
    AppendChar(buf, pos, '"');
    i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
      IF (s[i] = '"') OR (s[i] = '\') THEN
        AppendChar(buf, pos, '\')
      END;
      AppendChar(buf, pos, s[i]);
      INC(i)
    END;
    AppendChar(buf, pos, '"')
  END AppendJsonStr;

  PROCEDURE AppendComma(VAR buf: ARRAY OF CHAR;
                        VAR pos: CARDINAL;
                        VAR first: BOOLEAN);
  BEGIN
    IF NOT first THEN AppendChar(buf, pos, ',') END;
    first := FALSE
  END AppendComma;

  PROCEDURE BuildJwtPayload(VAR p: Principal;
                            VAR buf: ARRAY OF CHAR;
                            VAR len: CARDINAL): BOOLEAN;
  VAR pos: CARDINAL; first: BOOLEAN; i: CARDINAL;
  BEGIN
    pos := 0;
    first := TRUE;
    AppendChar(buf, pos, '{');

    IF p.subject[0] # 0C THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"sub":');
      AppendJsonStr(buf, pos, p.subject)
    END;
    IF p.issuer[0] # 0C THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"iss":');
      AppendJsonStr(buf, pos, p.issuer)
    END;
    IF p.audience[0] # 0C THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"aud":');
      AppendJsonStr(buf, pos, p.audience)
    END;
    IF p.expUnix # 0 THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"exp":');
      AppendLongInt(buf, pos, p.expUnix)
    END;
    IF p.nbfUnix # 0 THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"nbf":');
      AppendLongInt(buf, pos, p.nbfUnix)
    END;
    IF p.iatUnix # 0 THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"iat":');
      AppendLongInt(buf, pos, p.iatUnix)
    END;
    IF p.kid[0] # 0C THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"kid":');
      AppendJsonStr(buf, pos, p.kid)
    END;
    IF p.jti[0] # 0C THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"jti":');
      AppendJsonStr(buf, pos, p.jti)
    END;
    IF p.scopeCount > 0 THEN
      AppendComma(buf, pos, first);
      AppendLit(buf, pos, '"scope":"');
      FOR i := 0 TO p.scopeCount - 1 DO
        IF i > 0 THEN AppendChar(buf, pos, ' ') END;
        AppendStr(buf, pos, p.scopes[i])
      END;
      AppendChar(buf, pos, '"')
    END;
    IF p.claims.count > 0 THEN
      FOR i := 0 TO p.claims.count - 1 DO
        AppendComma(buf, pos, first);
        AppendJsonStr(buf, pos, p.claims.items[i].key);
        AppendChar(buf, pos, ':');
        CASE p.claims.items[i].vtype OF
          Str:
            AppendJsonStr(buf, pos, p.claims.items[i].s) |
          Int:
            AppendLongInt(buf, pos, p.claims.items[i].i) |
          Bool:
            IF p.claims.items[i].b THEN
              AppendLit(buf, pos, "true")
            ELSE
              AppendLit(buf, pos, "false")
            END
        END
      END
    END;

    AppendChar(buf, pos, '}');
    IF pos <= HIGH(buf) THEN buf[pos] := 0C END;
    len := pos;
    RETURN pos <= HIGH(buf)
  END BuildJwtPayload;

  (* ── JWT HS256 header (constant) ─────────────────── *)

  PROCEDURE BuildJwtHeader(VAR kid: ARRAY OF CHAR;
                           VAR buf: ARRAY OF CHAR;
                           VAR len: CARDINAL);
  VAR pos: CARDINAL;
  BEGIN
    pos := 0;
    AppendLit(buf, pos, '{"alg":"HS256","typ":"JWT"');
    IF kid[0] # 0C THEN
      AppendLit(buf, pos, ',"kid":');
      AppendJsonStr(buf, pos, kid)
    END;
    AppendChar(buf, pos, '}');
    IF pos <= HIGH(buf) THEN buf[pos] := 0C END;
    len := pos
  END BuildJwtHeader;

  (* ── JWT HS256 token operations ──────────────────── *)

  PROCEDURE SignHS256(kr: Keyring;
                      kid: ARRAY OF CHAR;
                      VAR p: Principal;
                      VAR token: ARRAY OF CHAR;
                      VAR tokenLen: CARDINAL): Status;
  VAR
    krp: POINTER TO KeyringRec;
    kidx: CARDINAL;
    headerJson: ARRAY [0..255] OF CHAR;
    headerLen: CARDINAL;
    payloadJson: DataBuf;
    payloadLen: CARDINAL;
    headerB64: ARRAY [0..511] OF CHAR;
    payloadB64: ARRAY [0..MaxRawTokenLen-1] OF CHAR;
    headerB64Len, payloadB64Len: INTEGER;
    sigInput: RawBuf;
    sigInputLen: CARDINAL;
    hmacOut: ARRAY [0..31] OF CHAR;
    sigB64: ARRAY [0..63] OF CHAR;
    sigB64Len: INTEGER;
    rc: INTEGER;
    i: CARDINAL;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    krp := kr;

    (* Find key *)
    IF kid[0] # 0C THEN
      IF NOT FindKey(kr, kid, kidx) THEN RETURN BadSignature END;
      IF krp^.keys[kidx].kind # KkHS256 THEN RETURN Invalid END
    ELSE
      IF NOT GetActiveKey(kr, kidx) THEN RETURN BadSignature END;
      IF krp^.keys[kidx].kind # KkHS256 THEN RETURN Invalid END
    END;

    (* Build header + payload JSON *)
    BuildJwtHeader(krp^.keys[kidx].kid, headerJson, headerLen);
    IF NOT BuildJwtPayload(p, payloadJson, payloadLen) THEN
      RETURN Invalid
    END;

    (* Base64url encode header *)
    headerB64Len := m2_auth_b64url_encode(
      ADR(headerJson), INTEGER(headerLen),
      ADR(headerB64), 511);
    IF headerB64Len < 0 THEN RETURN Invalid END;

    (* Base64url encode payload *)
    payloadB64Len := m2_auth_b64url_encode(
      ADR(payloadJson), INTEGER(payloadLen),
      ADR(payloadB64), MaxRawTokenLen - 1);
    IF payloadB64Len < 0 THEN RETURN Invalid END;

    (* Build signing input: header.payload *)
    sigInputLen := 0;
    FOR i := 0 TO CARDINAL(headerB64Len) - 1 DO
      sigInput[sigInputLen] := headerB64[i]; INC(sigInputLen)
    END;
    sigInput[sigInputLen] := '.'; INC(sigInputLen);
    FOR i := 0 TO CARDINAL(payloadB64Len) - 1 DO
      sigInput[sigInputLen] := payloadB64[i]; INC(sigInputLen)
    END;

    (* HMAC-SHA256 *)
    rc := m2_auth_hmac_sha256(
      ADR(krp^.keys[kidx].sym), 32,
      ADR(sigInput), INTEGER(sigInputLen),
      ADR(hmacOut));
    IF rc # 0 THEN RETURN BadSignature END;

    (* Base64url encode signature *)
    sigB64Len := m2_auth_b64url_encode(
      ADR(hmacOut), 32,
      ADR(sigB64), 63);
    IF sigB64Len < 0 THEN RETURN Invalid END;

    (* Assemble token: header.payload.signature *)
    tokenLen := 0;
    FOR i := 0 TO CARDINAL(headerB64Len) - 1 DO
      IF tokenLen <= HIGH(token) THEN
        token[tokenLen] := headerB64[i]; INC(tokenLen)
      END
    END;
    IF tokenLen <= HIGH(token) THEN
      token[tokenLen] := '.'; INC(tokenLen)
    END;
    FOR i := 0 TO CARDINAL(payloadB64Len) - 1 DO
      IF tokenLen <= HIGH(token) THEN
        token[tokenLen] := payloadB64[i]; INC(tokenLen)
      END
    END;
    IF tokenLen <= HIGH(token) THEN
      token[tokenLen] := '.'; INC(tokenLen)
    END;
    FOR i := 0 TO CARDINAL(sigB64Len) - 1 DO
      IF tokenLen <= HIGH(token) THEN
        token[tokenLen] := sigB64[i]; INC(tokenLen)
      END
    END;
    IF tokenLen <= HIGH(token) THEN
      token[tokenLen] := 0C
    END;

    RETURN OK
  END SignHS256;

  (* Split token at '.' characters.
     Returns TRUE if exactly 3 parts found. *)
  PROCEDURE SplitJwt(VAR token: ARRAY OF CHAR;
                     VAR p1Start, p1Len,
                         p2Start, p2Len,
                         p3Start, p3Len: CARDINAL): BOOLEAN;
  VAR i, dots, partStart: CARDINAL;
  BEGIN
    dots := 0;
    partStart := 0;
    i := 0;
    WHILE (i <= HIGH(token)) AND (token[i] # 0C) DO
      IF token[i] = '.' THEN
        IF dots = 0 THEN
          p1Start := partStart; p1Len := i - partStart
        ELSIF dots = 1 THEN
          p2Start := partStart; p2Len := i - partStart
        ELSE
          RETURN FALSE  (* too many dots *)
        END;
        INC(dots);
        partStart := i + 1
      END;
      INC(i)
    END;
    IF dots = 2 THEN
      p3Start := partStart; p3Len := i - partStart;
      RETURN TRUE
    END;
    RETURN FALSE
  END SplitJwt;

  PROCEDURE VerifyHS256(kr: Keyring;
                        VAR token: ARRAY OF CHAR;
                        clockSkew: CARDINAL;
                        audSet: BOOLEAN; VAR aud: ARRAY OF CHAR;
                        issSet: BOOLEAN; VAR iss: ARRAY OF CHAR;
                        VAR principal: Principal): Status;
  VAR
    krp: POINTER TO KeyringRec;
    p1S, p1L, p2S, p2L, p3S, p3L: CARDINAL;
    headerJson: ARRAY [0..255] OF CHAR;
    payloadJson: DataBuf;
    sigBytes: ARRAY [0..63] OF CHAR;
    headerDecLen, payloadDecLen, sigDecLen: INTEGER;
    hmacOut: ARRAY [0..31] OF CHAR;
    sigInputLen: CARDINAL;
    kidx: CARDINAL;
    rc: INTEGER;
    now: LONGINT;
    st: Status;
    algBuf: ARRAY [0..15] OF CHAR;
    kidBuf: KeyId;
    valLen: CARDINAL;
    pos: CARDINAL;
    keyBuf: ARRAY [0..15] OF CHAR;
    keyLen: CARDINAL;
  BEGIN
    (* Split into 3 parts *)
    IF NOT SplitJwt(token, p1S, p1L, p2S, p2L, p3S, p3L) THEN
      RETURN Invalid
    END;

    (* Decode header *)
    headerDecLen := m2_auth_b64url_decode(
      ADR(token[p1S]), INTEGER(p1L),
      ADR(headerJson), 255);
    IF headerDecLen < 0 THEN RETURN Invalid END;
    IF headerDecLen <= 255 THEN
      headerJson[headerDecLen] := 0C
    END;

    (* Parse header for alg and kid *)
    ClearStr(algBuf);
    ClearStr(kidBuf);
    pos := 0;
    SkipWhitespace(headerJson, pos, CARDINAL(headerDecLen));
    IF (pos < CARDINAL(headerDecLen)) AND (headerJson[pos] = '{') THEN
      INC(pos);
      LOOP
        SkipWhitespace(headerJson, pos, CARDINAL(headerDecLen));
        IF (pos >= CARDINAL(headerDecLen)) OR
           (headerJson[pos] = '}') THEN EXIT END;
        IF NOT ParseJsonString(headerJson, pos,
                               CARDINAL(headerDecLen),
                               keyBuf, keyLen) THEN EXIT END;
        SkipWhitespace(headerJson, pos, CARDINAL(headerDecLen));
        IF (pos < CARDINAL(headerDecLen)) AND
           (headerJson[pos] = ':') THEN INC(pos) END;
        SkipWhitespace(headerJson, pos, CARDINAL(headerDecLen));
        IF StrEqLit(keyBuf, "alg") THEN
          IF NOT ParseJsonString(headerJson, pos,
                                 CARDINAL(headerDecLen),
                                 algBuf, valLen) THEN
            RETURN Invalid
          END
        ELSIF StrEqLit(keyBuf, "kid") THEN
          IF NOT ParseJsonString(headerJson, pos,
                                 CARDINAL(headerDecLen),
                                 kidBuf, valLen) THEN
            RETURN Invalid
          END
        ELSE
          (* Skip unknown header value *)
          IF NOT ParseJsonString(headerJson, pos,
                                 CARDINAL(headerDecLen),
                                 keyBuf, keyLen) THEN
            (* Try skipping non-string value *)
            WHILE (pos < CARDINAL(headerDecLen)) AND
                  (headerJson[pos] # ',') AND
                  (headerJson[pos] # '}') DO
              INC(pos)
            END
          END
        END;
        SkipWhitespace(headerJson, pos, CARDINAL(headerDecLen));
        IF (pos < CARDINAL(headerDecLen)) AND
           (headerJson[pos] = ',') THEN INC(pos) END
      END
    END;

    (* Reject alg "none" *)
    IF StrEqLit(algBuf, "none") THEN RETURN BadSignature END;
    IF NOT StrEqLit(algBuf, "HS256") THEN RETURN Unsupported END;

    (* Look up key by kid or active key *)
    krp := kr;
    IF kidBuf[0] # 0C THEN
      IF NOT FindKey(kr, kidBuf, kidx) THEN RETURN BadSignature END
    ELSE
      IF NOT GetActiveKey(kr, kidx) THEN RETURN BadSignature END
    END;
    IF krp^.keys[kidx].kind # KkHS256 THEN RETURN BadSignature END;

    (* Compute HMAC over "header.payload" *)
    sigInputLen := p1L + 1 + p2L;
    rc := m2_auth_hmac_sha256(
      ADR(krp^.keys[kidx].sym), 32,
      ADR(token[p1S]), INTEGER(sigInputLen),
      ADR(hmacOut));
    IF rc # 0 THEN RETURN BadSignature END;

    (* Decode and compare signature *)
    sigDecLen := m2_auth_b64url_decode(
      ADR(token[p3S]), INTEGER(p3L),
      ADR(sigBytes), 63);
    IF sigDecLen # HmacLen THEN RETURN BadSignature END;

    rc := m2_auth_ct_compare(ADR(hmacOut), ADR(sigBytes), HmacLen);
    IF rc # 0 THEN RETURN BadSignature END;

    (* Decode payload *)
    payloadDecLen := m2_auth_b64url_decode(
      ADR(token[p2S]), INTEGER(p2L),
      ADR(payloadJson), MaxPayloadLen - 1);
    IF payloadDecLen < 0 THEN RETURN Invalid END;
    IF payloadDecLen < MaxPayloadLen THEN
      payloadJson[payloadDecLen] := 0C
    END;

    (* Parse payload into principal *)
    st := ParseJwtPayload(payloadJson, CARDINAL(payloadDecLen),
                          principal);
    IF st # OK THEN RETURN st END;

    (* Validate time claims *)
    now := m2_auth_get_unix_time();
    IF (principal.expUnix # 0) AND
       (now > principal.expUnix + LONGINT(clockSkew)) THEN
      RETURN Expired
    END;
    IF (principal.nbfUnix # 0) AND
       (now + LONGINT(clockSkew) < principal.nbfUnix) THEN
      RETURN NotYetValid
    END;

    (* Validate issuer *)
    IF issSet AND (iss[0] # 0C) THEN
      IF NOT StrEqual(principal.issuer, iss) THEN
        RETURN VerifyFailed
      END
    END;

    (* Validate audience *)
    IF audSet AND (aud[0] # 0C) THEN
      IF NOT StrEqual(principal.audience, aud) THEN
        RETURN VerifyFailed
      END
    END;

    RETURN OK
  END VerifyHS256;

  (* ── Ed25519 / PASETO-like token operations ─────── *)

  PROCEDURE SignEd25519(kr: Keyring;
                        kid: ARRAY OF CHAR;
                        VAR p: Principal;
                        VAR token: ARRAY OF CHAR;
                        VAR tokenLen: CARDINAL): Status;
  VAR
    krp: POINTER TO KeyringRec;
    kidx: CARDINAL;
    payloadJson: DataBuf;
    payloadLen: CARDINAL;
    content: ARRAY [0..MaxPayloadLen+63] OF CHAR;
    sigOut: ARRAY [0..63] OF CHAR;
    contentB64: ARRAY [0..MaxRawTokenLen-1] OF CHAR;
    contentB64Len: INTEGER;
    rc: INTEGER;
    i: CARDINAL;
    prefix: ARRAY [0..11] OF CHAR;
  BEGIN
    IF kr = NIL THEN RETURN Invalid END;
    IF m2_auth_has_ed25519() = 0 THEN RETURN Unsupported END;

    krp := kr;
    IF kid[0] # 0C THEN
      IF NOT FindKey(kr, kid, kidx) THEN RETURN BadSignature END
    ELSE
      IF NOT GetActiveKey(kr, kidx) THEN RETURN BadSignature END
    END;
    (* Ed25519 keys need a private key — we store pub only in keyring.
       For signing, the caller must provide a private key via a
       separate mechanism.  For now, return Unsupported for Ed25519
       signing via the keyring.  The test suite uses bridge directly. *)
    RETURN Unsupported
  END SignEd25519;

  (* ── Token dispatch ──────────────────────────────── *)

  PROCEDURE VerifyBearerToken(v: Verifier;
                               token: ARRAY OF CHAR;
                               VAR principal: Principal): Status;
  VAR
    vp: POINTER TO VerifierRec;
  BEGIN
    IF v = NIL THEN RETURN Invalid END;
    vp := v;

    (* Detect token type: PASETO starts with "v4.public." *)
    IF (StrLen(token) > 10) AND
       (token[0] = 'v') AND (token[1] = '4') AND
       (token[2] = '.') AND (token[3] = 'p') THEN
      (* Ed25519 / PASETO path — not implemented in this release *)
      RETURN Unsupported
    END;

    (* JWT path *)
    RETURN VerifyHS256(vp^.kr, token,
                       vp^.clockSkew,
                       vp^.audSet, vp^.aud,
                       vp^.issSet, vp^.iss,
                       principal)
  END VerifyBearerToken;

  PROCEDURE SignToken(kr: Keyring;
                       kind: TokenKind;
                       kid: ARRAY OF CHAR;
                       VAR principal: Principal;
                       VAR token: ARRAY OF CHAR;
                       VAR tokenLen: CARDINAL): Status;
  BEGIN
    CASE kind OF
      JwtHS256:
        RETURN SignHS256(kr, kid, principal, token, tokenLen) |
      PasetoV4Public:
        RETURN SignEd25519(kr, kid, principal, token, tokenLen) |
      JwtES256:
        RETURN Unsupported
    END
  END SignToken;

  (* ── Hex key helpers ────────────────────────────── *)

  PROCEDURE HexNibble(ch: CHAR; VAR val: CARDINAL): BOOLEAN;
  BEGIN
    IF (ch >= '0') AND (ch <= '9') THEN
      val := ORD(ch) - ORD('0'); RETURN TRUE
    ELSIF (ch >= 'a') AND (ch <= 'f') THEN
      val := ORD(ch) - ORD('a') + 10; RETURN TRUE
    ELSIF (ch >= 'A') AND (ch <= 'F') THEN
      val := ORD(ch) - ORD('A') + 10; RETURN TRUE
    END;
    RETURN FALSE
  END HexNibble;

  PROCEDURE DecodeHexKey(VAR hex: ARRAY OF CHAR;
                          VAR key: SymKey): Status;
  VAR i, hi, lo: CARDINAL;
  BEGIN
    IF Length(hex) # 64 THEN RETURN Invalid END;
    FOR i := 0 TO 31 DO
      IF NOT HexNibble(hex[i * 2], hi) THEN RETURN Invalid END;
      IF NOT HexNibble(hex[i * 2 + 1], lo) THEN RETURN Invalid END;
      key[i] := CHR(hi * 16 + lo)
    END;
    RETURN OK
  END DecodeHexKey;

  PROCEDURE QuickSignHS256(VAR hexSecret: ARRAY OF CHAR;
                            sub: ARRAY OF CHAR;
                            ttl: CARDINAL;
                            VAR token: ARRAY OF CHAR;
                            VAR tokenLen: CARDINAL): Status;
  VAR
    key: SymKey;
    kr: Keyring;
    p: Principal;
    st, st2: Status;
    kid: ARRAY [0..0] OF CHAR;
    now: LONGINT;
  BEGIN
    st := DecodeHexKey(hexSecret, key);
    IF st # OK THEN RETURN st END;
    st := KeyringCreate(kr);
    IF st # OK THEN RETURN st END;
    kid[0] := 0C;
    st := KeyringAddHS256(kr, kid, key);
    IF st # OK THEN
      st2 := KeyringDestroy(kr);
      RETURN st
    END;
    InitPrincipal(p);
    Assign(sub, p.subject);
    now := m2_auth_get_unix_time();
    p.iatUnix := now;
    p.expUnix := now + VAL(LONGINT, ttl);
    st := SignToken(kr, JwtHS256, kid, p, token, tokenLen);
    st2 := KeyringDestroy(kr);
    IF (st = OK) AND (tokenLen <= HIGH(token)) THEN
      token[tokenLen] := 0C
    END;
    RETURN st
  END QuickSignHS256;

  (* ── Replay cache ────────────────────────────────── *)

  PROCEDURE ReplayCacheCreate(VAR rc: ReplayCache): Status;
  VAR p: POINTER TO ReplayCacheRec; i: CARDINAL;
  BEGIN
    ALLOCATE(p, TSIZE(ReplayCacheRec));
    IF p = NIL THEN rc := NIL; RETURN OutOfMemory END;
    p^.count := 0;
    p^.next := 0;
    FOR i := 0 TO MaxReplay - 1 DO
      p^.entries[i].used := FALSE;
      ClearStr(p^.entries[i].jti)
    END;
    rc := p;
    RETURN OK
  END ReplayCacheCreate;

  PROCEDURE ReplayCacheDestroy(VAR rc: ReplayCache): Status;
  VAR p: POINTER TO ReplayCacheRec;
  BEGIN
    IF rc = NIL THEN RETURN Invalid END;
    p := rc;
    DEALLOCATE(p, TSIZE(ReplayCacheRec));
    rc := NIL;
    RETURN OK
  END ReplayCacheDestroy;

  PROCEDURE ReplayCacheSeenOrAdd(rc: ReplayCache;
                                  jti: ARRAY OF CHAR;
                                  expUnix: LONGINT): Status;
  VAR
    p: POINTER TO ReplayCacheRec;
    i, slot: CARDINAL;
    now: LONGINT;
  BEGIN
    IF rc = NIL THEN RETURN Invalid END;
    p := rc;
    now := m2_auth_get_unix_time();

    (* Check for existing entry *)
    FOR i := 0 TO MaxReplay - 1 DO
      IF p^.entries[i].used THEN
        (* Evict expired entries *)
        IF p^.entries[i].expUnix < now THEN
          p^.entries[i].used := FALSE
        ELSIF StrEqual(p^.entries[i].jti, jti) THEN
          RETURN VerifyFailed  (* replay detected *)
        END
      END
    END;

    (* Add new entry at next slot (ring buffer) *)
    slot := p^.next;
    StrCopy(jti, p^.entries[slot].jti);
    p^.entries[slot].expUnix := expUnix;
    p^.entries[slot].used := TRUE;
    p^.next := (slot + 1) MOD MaxReplay;
    IF p^.count < MaxReplay THEN INC(p^.count) END;

    RETURN OK
  END ReplayCacheSeenOrAdd;

  (* ── Policy ──────────────────────────────────────── *)

  PROCEDURE PolicyCreate(VAR pol: Policy): Status;
  VAR p: POINTER TO PolicyRec;
  BEGIN
    ALLOCATE(p, TSIZE(PolicyRec));
    IF p = NIL THEN pol := NIL; RETURN OutOfMemory END;
    p^.count := 0;
    pol := p;
    RETURN OK
  END PolicyCreate;

  PROCEDURE PolicyDestroy(VAR pol: Policy): Status;
  VAR p: POINTER TO PolicyRec;
  BEGIN
    IF pol = NIL THEN RETURN Invalid END;
    p := pol;
    DEALLOCATE(p, TSIZE(PolicyRec));
    pol := NIL;
    RETURN OK
  END PolicyDestroy;

  PROCEDURE PolicyAllowScope(pol: Policy;
                              scope: ARRAY OF CHAR): Status;
  VAR p: POINTER TO PolicyRec; idx: CARDINAL;
  BEGIN
    IF pol = NIL THEN RETURN Invalid END;
    p := pol;
    IF p^.count >= MaxPolicyRules THEN RETURN OutOfMemory END;
    idx := p^.count;
    p^.rules[idx].kind := PrScope;
    StrCopy(scope, p^.rules[idx].scope);
    ClearStr(p^.rules[idx].key);
    ClearStr(p^.rules[idx].value);
    INC(p^.count);
    RETURN OK
  END PolicyAllowScope;

  PROCEDURE PolicyAllowClaimEquals(pol: Policy;
                                    key: ARRAY OF CHAR;
                                    value: ARRAY OF CHAR): Status;
  VAR p: POINTER TO PolicyRec; idx: CARDINAL;
  BEGIN
    IF pol = NIL THEN RETURN Invalid END;
    p := pol;
    IF p^.count >= MaxPolicyRules THEN RETURN OutOfMemory END;
    idx := p^.count;
    p^.rules[idx].kind := PrClaim;
    ClearStr(p^.rules[idx].scope);
    StrCopy(key, p^.rules[idx].key);
    StrCopy(value, p^.rules[idx].value);
    INC(p^.count);
    RETURN OK
  END PolicyAllowClaimEquals;

  PROCEDURE PrincipalHasScope(VAR p: Principal;
                               VAR scope: ARRAY OF CHAR): BOOLEAN;
  VAR i: CARDINAL;
  BEGIN
    IF p.scopeCount = 0 THEN RETURN FALSE END;
    FOR i := 0 TO p.scopeCount - 1 DO
      IF StrEqual(p.scopes[i], scope) THEN RETURN TRUE END
    END;
    RETURN FALSE
  END PrincipalHasScope;

  PROCEDURE PrincipalClaimEquals(VAR p: Principal;
                                  VAR key, value: ARRAY OF CHAR): BOOLEAN;
  VAR i: CARDINAL;
  BEGIN
    IF p.claims.count = 0 THEN RETURN FALSE END;
    FOR i := 0 TO p.claims.count - 1 DO
      IF StrEqual(p.claims.items[i].key, key) AND
         (p.claims.items[i].vtype = Str) AND
         StrEqual(p.claims.items[i].s, value) THEN
        RETURN TRUE
      END
    END;
    RETURN FALSE
  END PrincipalClaimEquals;

  PROCEDURE Authorize(pol: Policy;
                       VAR principal: Principal): Status;
  VAR
    pp: POINTER TO PolicyRec;
    i: CARDINAL;
    matched: BOOLEAN;
  BEGIN
    IF pol = NIL THEN RETURN Invalid END;
    pp := pol;
    IF pp^.count = 0 THEN RETURN Denied END;

    (* ANY rule match → Allow *)
    FOR i := 0 TO pp^.count - 1 DO
      matched := FALSE;
      CASE pp^.rules[i].kind OF
        PrScope:
          matched := PrincipalHasScope(principal,
                                        pp^.rules[i].scope) |
        PrClaim:
          matched := PrincipalClaimEquals(principal,
                                           pp^.rules[i].key,
                                           pp^.rules[i].value)
      END;
      IF matched THEN RETURN OK END
    END;

    RETURN Denied
  END Authorize;

BEGIN
  m2_auth_init
END Auth.
