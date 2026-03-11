IMPLEMENTATION MODULE Oidc;

(* OIDC provider — RS256 JWT verification + claim extraction.

   Uses m2json SAX parser for JSON claim parsing, AuthBridge for
   base64url decoding and Unix time, Jwks for key management and
   RS256 verification. *)

FROM SYSTEM IMPORT ADR, ADDRESS, LONGCARD, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Strings IMPORT Assign;
IMPORT Json;
FROM Jwks IMPORT JkOk, JkNoSuchKid, VerifyRS256, FindKey;
FROM AuthBridge IMPORT m2_auth_b64url_decode, m2_auth_get_unix_time;

(* ── Internal types ──────────────────────────────────── *)

TYPE
  ProvRec = RECORD
    issuer:     ARRAY [0..MaxIssLen] OF CHAR;
    clientId:   ARRAY [0..MaxAudLen] OF CHAR;
    clockSkew:  CARDINAL;
    keySet:     ADDRESS;  (* Jwks.KeySet *)
  END;

  ProvPtr = POINTER TO ProvRec;

(* ── String helpers ──────────────────────────────────── *)

PROCEDURE StrEq(VAR a, b: ARRAY OF CHAR): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(a)) AND (i <= HIGH(b)) AND
        (a[i] # 0C) AND (b[i] # 0C) DO
    IF a[i] # b[i] THEN RETURN FALSE END;
    INC(i)
  END;
  IF (i <= HIGH(a)) AND (a[i] # 0C) THEN RETURN FALSE END;
  IF (i <= HIGH(b)) AND (b[i] # 0C) THEN RETURN FALSE END;
  RETURN TRUE
END StrEq;

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
  RETURN i
END StrLen;

PROCEDURE CopyN(src: ADDRESS; srcLen: CARDINAL;
                 VAR dst: ARRAY OF CHAR);
TYPE CharPtr = POINTER TO CHAR;
VAR
  i, max: CARDINAL;
  p: CharPtr;
BEGIN
  max := HIGH(dst);
  IF srcLen < max THEN max := srcLen END;
  i := 0;
  WHILE i < max DO
    p := CharPtr(LONGCARD(src) + LONGCARD(i));
    dst[i] := p^;
    INC(i)
  END;
  IF i <= HIGH(dst) THEN dst[i] := 0C END
END CopyN;

(* ── Init claims ─────────────────────────────────────── *)

PROCEDURE InitClaims(VAR c: OidcClaims);
VAR i: CARDINAL;
BEGIN
  c.subject[0] := 0C;
  c.issuer[0] := 0C;
  c.audience[0] := 0C;
  c.username[0] := 0C;
  c.email[0] := 0C;
  c.azp[0] := 0C;
  c.expUnix := 0;
  c.nbfUnix := 0;
  c.iatUnix := 0;
  c.roleCount := 0;
  c.groupCount := 0;
  i := 0;
  WHILE i < MaxRoles DO
    c.roles[i][0] := 0C;
    INC(i)
  END;
  i := 0;
  WHILE i < MaxGroups DO
    c.groups[i][0] := 0C;
    INC(i)
  END
END InitClaims;

(* ── Provider lifecycle ──────────────────────────────── *)

PROCEDURE CreateProvider(VAR prov: Provider;
                         VAR issuer, clientId: ARRAY OF CHAR;
                         clockSkewSecs: CARDINAL;
                         keySet: ADDRESS): Status;
VAR pp: ProvPtr;
BEGIN
  ALLOCATE(pp, TSIZE(ProvRec));
  IF pp = NIL THEN
    prov := NIL;
    RETURN OcOutOfMemory
  END;
  Assign(issuer, pp^.issuer);
  Assign(clientId, pp^.clientId);
  pp^.clockSkew := clockSkewSecs;
  pp^.keySet := keySet;
  prov := ADDRESS(pp);
  RETURN OcOk
END CreateProvider;

PROCEDURE DestroyProvider(VAR prov: Provider): Status;
VAR pp: ProvPtr;
BEGIN
  IF prov = NIL THEN RETURN OcInvalid END;
  pp := ProvPtr(prov);
  DEALLOCATE(pp, TSIZE(ProvRec));
  prov := NIL;
  RETURN OcOk
END DestroyProvider;

PROCEDURE GetKeySet(prov: Provider): ADDRESS;
VAR pp: ProvPtr;
BEGIN
  IF prov = NIL THEN RETURN NIL END;
  pp := ProvPtr(prov);
  RETURN pp^.keySet
END GetKeySet;

(* ── Discovery parsing ───────────────────────────────── *)

PROCEDURE ParseDiscovery(json: ADDRESS; jsonLen: CARDINAL;
                         VAR issuerOut: ARRAY OF CHAR;
                         VAR jwksUriOut: ARRAY OF CHAR): Status;
VAR
  p:   Json.Parser;
  tok: Json.Token;
  key: ARRAY [0..63] OF CHAR;
  hasIssuer, hasJwksUri: BOOLEAN;
  depth: INTEGER;
  dummy: BOOLEAN;
BEGIN
  issuerOut[0] := 0C;
  jwksUriOut[0] := 0C;
  hasIssuer := FALSE;
  hasJwksUri := FALSE;

  Json.Init(p, json, jsonLen);

  IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
  IF tok.kind # Json.JObjectStart THEN RETURN OcParseFailed END;

  LOOP
    IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
    IF tok.kind = Json.JObjectEnd THEN EXIT END;
    IF tok.kind = Json.JComma THEN
      (* skip *)
    ELSIF tok.kind = Json.JString THEN
      dummy := Json.GetString(p, tok, key);
      IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
      IF tok.kind # Json.JColon THEN RETURN OcParseFailed END;
      IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;

      IF StrEq(key, "issuer") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, issuerOut);
          hasIssuer := TRUE
        END
      ELSIF StrEq(key, "jwks_uri") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, jwksUriOut);
          hasJwksUri := TRUE
        END
      ELSE
        (* Value already consumed by Next above.  For nested
           objects/arrays we must drain them inline because
           Json.Skip would call Next again, losing a token. *)
        IF tok.kind = Json.JObjectStart THEN
          depth := 1;
          WHILE (depth > 0) AND Json.Next(p, tok) DO
            IF tok.kind = Json.JObjectStart THEN INC(depth)
            ELSIF tok.kind = Json.JObjectEnd THEN DEC(depth)
            END
          END
        ELSIF tok.kind = Json.JArrayStart THEN
          depth := 1;
          WHILE (depth > 0) AND Json.Next(p, tok) DO
            IF tok.kind = Json.JArrayStart THEN INC(depth)
            ELSIF tok.kind = Json.JArrayEnd THEN DEC(depth)
            END
          END
        END
      END
    END
  END;

  IF hasIssuer AND hasJwksUri THEN
    RETURN OcOk
  ELSE
    RETURN OcParseFailed
  END
END ParseDiscovery;

(* ── JWT splitting ───────────────────────────────────── *)

(* Find the positions of the two dots in a JWT.
   Returns TRUE if exactly two dots found. *)
PROCEDURE SplitJwt(VAR token: ARRAY OF CHAR;
                   VAR dot1, dot2, tLen: CARDINAL): BOOLEAN;
VAR i, dots: CARDINAL;
BEGIN
  dots := 0;
  i := 0;
  WHILE (i <= HIGH(token)) AND (token[i] # 0C) DO
    IF token[i] = '.' THEN
      INC(dots);
      IF dots = 1 THEN dot1 := i
      ELSIF dots = 2 THEN dot2 := i
      END
    END;
    INC(i)
  END;
  tLen := i;
  RETURN dots = 2
END SplitJwt;

(* ── PeekAlg ─────────────────────────────────────────── *)

PROCEDURE PeekAlg(VAR token: ARRAY OF CHAR;
                  VAR algOut: ARRAY OF CHAR): Status;
VAR
  dot1, dot2, tLen: CARDINAL;
  hdrBuf: ARRAY [0..511] OF CHAR;
  hdrLen: INTEGER;
  p:      Json.Parser;
  tok:    Json.Token;
  key:    ARRAY [0..31] OF CHAR;
  dummy:  BOOLEAN;
BEGIN
  algOut[0] := 0C;

  IF NOT SplitJwt(token, dot1, dot2, tLen) THEN
    RETURN OcInvalid
  END;

  (* Decode header *)
  hdrLen := m2_auth_b64url_decode(ADR(token), VAL(INTEGER, dot1),
                                   ADR(hdrBuf), 512);
  IF hdrLen <= 0 THEN RETURN OcInvalid END;

  (* Parse header JSON for "alg" *)
  Json.Init(p, ADR(hdrBuf), VAL(CARDINAL, hdrLen));
  IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
  IF tok.kind # Json.JObjectStart THEN RETURN OcParseFailed END;

  LOOP
    IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
    IF tok.kind = Json.JObjectEnd THEN EXIT END;
    IF tok.kind = Json.JComma THEN
      (* skip *)
    ELSIF tok.kind = Json.JString THEN
      dummy := Json.GetString(p, tok, key);
      IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
      IF tok.kind # Json.JColon THEN RETURN OcParseFailed END;
      IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
      IF StrEq(key, "alg") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, algOut);
          RETURN OcOk
        END
      ELSE
        IF (tok.kind = Json.JObjectStart) OR (tok.kind = Json.JArrayStart) THEN
          Json.Skip(p)
        END
      END
    END
  END;

  RETURN OcParseFailed  (* no alg found *)
END PeekAlg;

(* ── Parse roles from realm_access.roles array ─────── *)

PROCEDURE ParseRolesArray(VAR p: Json.Parser;
                          VAR claims: OidcClaims);
VAR tok: Json.Token; dummy: BOOLEAN;
BEGIN
  (* We are positioned right after JArrayStart *)
  LOOP
    IF NOT Json.Next(p, tok) THEN RETURN END;
    IF tok.kind = Json.JArrayEnd THEN RETURN END;
    IF tok.kind = Json.JComma THEN
      (* skip *)
    ELSIF tok.kind = Json.JString THEN
      IF claims.roleCount < MaxRoles THEN
        dummy := Json.GetString(p, tok,
                   claims.roles[claims.roleCount]);
        INC(claims.roleCount)
      END
    END
  END
END ParseRolesArray;

(* ── Parse groups array ──────────────────────────────── *)

PROCEDURE ParseGroupsArray(VAR p: Json.Parser;
                           VAR claims: OidcClaims);
VAR tok: Json.Token; dummy: BOOLEAN;
BEGIN
  LOOP
    IF NOT Json.Next(p, tok) THEN RETURN END;
    IF tok.kind = Json.JArrayEnd THEN RETURN END;
    IF tok.kind = Json.JComma THEN
      (* skip *)
    ELSIF tok.kind = Json.JString THEN
      IF claims.groupCount < MaxGroups THEN
        dummy := Json.GetString(p, tok,
                   claims.groups[claims.groupCount]);
        INC(claims.groupCount)
      END
    END
  END
END ParseGroupsArray;

(* ── Parse realm_access object for roles ─────────────── *)

PROCEDURE ParseRealmAccess(VAR p: Json.Parser;
                           VAR claims: OidcClaims);
VAR tok: Json.Token; key: ARRAY [0..31] OF CHAR; dummy: BOOLEAN;
BEGIN
  (* Positioned after JObjectStart of realm_access *)
  LOOP
    IF NOT Json.Next(p, tok) THEN RETURN END;
    IF tok.kind = Json.JObjectEnd THEN RETURN END;
    IF tok.kind = Json.JComma THEN
      (* skip *)
    ELSIF tok.kind = Json.JString THEN
      dummy := Json.GetString(p, tok, key);
      IF NOT Json.Next(p, tok) THEN RETURN END;
      IF tok.kind # Json.JColon THEN RETURN END;
      IF StrEq(key, "roles") THEN
        IF NOT Json.Next(p, tok) THEN RETURN END;
        IF tok.kind = Json.JArrayStart THEN
          ParseRolesArray(p, claims)
        END
      ELSE
        Json.Skip(p)
      END
    END
  END
END ParseRealmAccess;

(* ── Parse JWT payload claims ────────────────────────── *)

PROCEDURE ParsePayloadClaims(payBuf: ADDRESS; payLen: CARDINAL;
                             VAR claims: OidcClaims): BOOLEAN;
VAR
  p:     Json.Parser;
  tok:   Json.Token;
  key:   ARRAY [0..63] OF CHAR;
  tmpStr: ARRAY [0..MaxIssLen] OF CHAR;
  dummy: BOOLEAN;
BEGIN
  Json.Init(p, payBuf, payLen);
  IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
  IF tok.kind # Json.JObjectStart THEN RETURN FALSE END;

  LOOP
    IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
    IF tok.kind = Json.JObjectEnd THEN EXIT END;
    IF tok.kind = Json.JComma THEN
      (* skip *)
    ELSIF tok.kind = Json.JString THEN
      dummy := Json.GetString(p, tok, key);
      IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
      IF tok.kind # Json.JColon THEN RETURN FALSE END;

      IF StrEq(key, "sub") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, claims.subject)
        END

      ELSIF StrEq(key, "iss") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, claims.issuer)
        END

      ELSIF StrEq(key, "aud") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JString THEN
          (* aud as string *)
          dummy := Json.GetString(p, tok, claims.audience)
        ELSIF tok.kind = Json.JArrayStart THEN
          (* aud as array — take first element *)
          LOOP
            IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
            IF tok.kind = Json.JArrayEnd THEN EXIT END;
            IF tok.kind = Json.JComma THEN (* skip *) END;
            IF tok.kind = Json.JString THEN
              IF claims.audience[0] = 0C THEN
                dummy := Json.GetString(p, tok, claims.audience)
              END
            END
          END
        END

      ELSIF StrEq(key, "azp") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, claims.azp)
        END

      ELSIF StrEq(key, "exp") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JNumber THEN
          dummy := Json.GetLong(p, tok, claims.expUnix)
        END

      ELSIF StrEq(key, "nbf") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JNumber THEN
          dummy := Json.GetLong(p, tok, claims.nbfUnix)
        END

      ELSIF StrEq(key, "iat") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JNumber THEN
          dummy := Json.GetLong(p, tok, claims.iatUnix)
        END

      ELSIF StrEq(key, "preferred_username") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, claims.username)
        END

      ELSIF StrEq(key, "email") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, claims.email)
        END

      ELSIF StrEq(key, "realm_access") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JObjectStart THEN
          ParseRealmAccess(p, claims)
        ELSE
          IF (tok.kind = Json.JArrayStart) THEN
            Json.Skip(p)
          END
        END

      ELSIF StrEq(key, "groups") THEN
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JArrayStart THEN
          ParseGroupsArray(p, claims)
        END

      ELSIF StrEq(key, "role") THEN
        (* Single role claim (SPECTRA convention) — add to roles *)
        IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
        IF tok.kind = Json.JString THEN
          IF claims.roleCount < MaxRoles THEN
            dummy := Json.GetString(p, tok,
                       claims.roles[claims.roleCount]);
            INC(claims.roleCount)
          END
        END

      ELSE
        (* Skip unknown claim *)
        Json.Skip(p)
      END
    END
  END;

  RETURN TRUE
END ParsePayloadClaims;

(* ── VerifyToken ─────────────────────────────────────── *)

PROCEDURE VerifyToken(prov: Provider;
                      VAR token: ARRAY OF CHAR;
                      VAR claims: OidcClaims): Status;
VAR
  pp:      ProvPtr;
  dot1, dot2, tLen: CARDINAL;
  hdrBuf:  ARRAY [0..511] OF CHAR;
  hdrLen:  INTEGER;
  payBuf:  ARRAY [0..8191] OF CHAR;
  payLen:  INTEGER;
  sigBuf:  ARRAY [0..511] OF CHAR;
  sigLen:  INTEGER;
  p:       Json.Parser;
  tok:     Json.Token;
  key:     ARRAY [0..31] OF CHAR;
  algBuf:  ARRAY [0..MaxAlgLen] OF CHAR;
  kidBuf:  ARRAY [0..63] OF CHAR;   (* matches Jwks.MaxKidLen *)
  hasAlg, hasKid: BOOLEAN;
  now:     LONGINT;
  dummy:   BOOLEAN;
BEGIN
  IF prov = NIL THEN RETURN OcInvalid END;
  pp := ProvPtr(prov);
  InitClaims(claims);

  (* Split token at dots *)
  IF NOT SplitJwt(token, dot1, dot2, tLen) THEN
    RETURN OcInvalid
  END;

  (* ── 1. Decode and parse header ────────────────────── *)
  hdrLen := m2_auth_b64url_decode(ADR(token), VAL(INTEGER, dot1),
                                   ADR(hdrBuf), 512);
  IF hdrLen <= 0 THEN RETURN OcInvalid END;

  algBuf[0] := 0C;
  kidBuf[0] := 0C;
  hasAlg := FALSE;
  hasKid := FALSE;

  Json.Init(p, ADR(hdrBuf), VAL(CARDINAL, hdrLen));
  IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
  IF tok.kind # Json.JObjectStart THEN RETURN OcParseFailed END;

  LOOP
    IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
    IF tok.kind = Json.JObjectEnd THEN EXIT END;
    IF tok.kind = Json.JComma THEN (* skip *) END;
    IF tok.kind = Json.JString THEN
      dummy := Json.GetString(p, tok, key);
      IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
      IF tok.kind # Json.JColon THEN RETURN OcParseFailed END;
      IF NOT Json.Next(p, tok) THEN RETURN OcParseFailed END;
      IF StrEq(key, "alg") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, algBuf);
          hasAlg := TRUE
        END
      ELSIF StrEq(key, "kid") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, kidBuf);
          hasKid := TRUE
        END
      ELSE
        IF (tok.kind = Json.JObjectStart) OR (tok.kind = Json.JArrayStart) THEN
          Json.Skip(p)
        END
      END
    END
  END;

  (* Validate algorithm *)
  IF NOT hasAlg THEN RETURN OcUnsupportedAlg END;
  IF NOT StrEq(algBuf, "RS256") THEN RETURN OcUnsupportedAlg END;
  IF NOT hasKid THEN RETURN OcNoKid END;

  (* ── 2. Decode signature ───────────────────────────── *)
  sigLen := m2_auth_b64url_decode(
    ADR(token[dot2 + 1]),
    VAL(INTEGER, tLen - dot2 - 1),
    ADR(sigBuf), 512);
  IF sigLen <= 0 THEN RETURN OcInvalid END;

  (* ── 3. Verify RS256 signature ─────────────────────── *)
  IF FindKey(pp^.keySet, kidBuf) # JkOk THEN
    RETURN OcKeyNotFound
  END;

  (* Signing input is the raw base64url text "header.payload" *)
  IF VerifyRS256(pp^.keySet, kidBuf,
                  ADR(token), dot2,
                  ADR(sigBuf), VAL(CARDINAL, sigLen)) # JkOk THEN
    RETURN OcBadSignature
  END;

  (* ── 4. Decode and parse payload ───────────────────── *)
  payLen := m2_auth_b64url_decode(
    ADR(token[dot1 + 1]),
    VAL(INTEGER, dot2 - dot1 - 1),
    ADR(payBuf), 8192);
  IF payLen <= 0 THEN RETURN OcParseFailed END;

  IF NOT ParsePayloadClaims(ADR(payBuf), VAL(CARDINAL, payLen),
                             claims) THEN
    RETURN OcParseFailed
  END;

  (* ── 5. Validate standard claims ───────────────────── *)

  (* Check issuer *)
  IF NOT StrEq(claims.issuer, pp^.issuer) THEN
    RETURN OcBadIssuer
  END;

  (* Check audience — match against clientId or azp *)
  IF NOT StrEq(claims.audience, pp^.clientId) THEN
    IF (claims.azp[0] = 0C) OR
       NOT StrEq(claims.azp, pp^.clientId) THEN
      RETURN OcBadAudience
    END
  END;

  (* Check expiry *)
  now := m2_auth_get_unix_time();
  IF claims.expUnix > 0 THEN
    IF now > claims.expUnix + VAL(LONGINT, pp^.clockSkew) THEN
      RETURN OcExpired
    END
  END;

  (* Check not-before *)
  IF claims.nbfUnix > 0 THEN
    IF now + VAL(LONGINT, pp^.clockSkew) < claims.nbfUnix THEN
      RETURN OcNotYetValid
    END
  END;

  RETURN OcOk
END VerifyToken;

END Oidc.
