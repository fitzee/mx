IMPLEMENTATION MODULE Jwks;

(* JWKS key set management — parses JWKS JSON, stores RSA keys by kid,
   provides thread-safe RS256 verification.

   Uses m2json SAX parser for JWKS parsing, AuthBridge for base64url
   decode, OidcBridge for RSA key construction and verification,
   and Threads (m2pthreads) for mutex protection. *)

FROM SYSTEM IMPORT ADR, ADDRESS, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Strings IMPORT Assign;
IMPORT Json;
FROM OidcBridge IMPORT m2_oidc_rsa_from_ne, m2_oidc_rsa_verify,
                        m2_oidc_rsa_free;
FROM AuthBridge IMPORT m2_auth_b64url_decode;
FROM Threads IMPORT Mutex, MutexInit, MutexDestroy,
                     MutexLock, MutexUnlock;

(* ── Internal types ──────────────────────────────────── *)

TYPE
  KeyEntry = RECORD
    kid:   ARRAY [0..MaxKidLen] OF CHAR;
    key:   ADDRESS;   (* EVP_PKEY* from OidcBridge *)
    valid: BOOLEAN;
  END;

  KeySetRec = RECORD
    entries: ARRAY [0..MaxKeys-1] OF KeyEntry;
    count:   CARDINAL;
    mu:      Mutex;
  END;

  KeySetPtr = POINTER TO KeySetRec;

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
  (* Both must be at NUL or end *)
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

(* ── Clear all keys (caller must hold lock) ──────────── *)

PROCEDURE ClearKeys(ksp: KeySetPtr);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < ksp^.count DO
    IF ksp^.entries[i].valid AND (ksp^.entries[i].key # NIL) THEN
      m2_oidc_rsa_free(ksp^.entries[i].key)
    END;
    ksp^.entries[i].valid := FALSE;
    ksp^.entries[i].key := NIL;
    ksp^.entries[i].kid[0] := 0C;
    INC(i)
  END;
  ksp^.count := 0
END ClearKeys;

(* ── Lifecycle ───────────────────────────────────────── *)

PROCEDURE Create(VAR ks: KeySet): Status;
VAR ksp: KeySetPtr; i: CARDINAL;
BEGIN
  ALLOCATE(ksp, TSIZE(KeySetRec));
  IF ksp = NIL THEN
    ks := NIL;
    RETURN JkOutOfMemory
  END;
  ksp^.count := 0;
  i := 0;
  WHILE i < MaxKeys DO
    ksp^.entries[i].valid := FALSE;
    ksp^.entries[i].key := NIL;
    ksp^.entries[i].kid[0] := 0C;
    INC(i)
  END;
  MutexInit(ksp^.mu);
  ks := ADDRESS(ksp);
  RETURN JkOk
END Create;

PROCEDURE Destroy(VAR ks: KeySet): Status;
VAR ksp: KeySetPtr;
BEGIN
  IF ks = NIL THEN RETURN JkInvalid END;
  ksp := KeySetPtr(ks);
  MutexLock(ksp^.mu);
  ClearKeys(ksp);
  MutexUnlock(ksp^.mu);
  MutexDestroy(ksp^.mu);
  DEALLOCATE(ksp, TSIZE(KeySetRec));
  ks := NIL;
  RETURN JkOk
END Destroy;

(* ── JWKS JSON Parsing ───────────────────────────────── *)

(* Parse a single JWK object from the SAX parser.
   Extracts kty, alg, kid, n, e fields.
   Returns TRUE if this is a usable RSA/RS256 key. *)

PROCEDURE ParseOneKey(VAR p: Json.Parser;
                      VAR kidOut: ARRAY OF CHAR;
                      VAR rsaKey: ADDRESS): BOOLEAN;
VAR
  tok:    Json.Token;
  key:    ARRAY [0..31] OF CHAR;
  kty:    ARRAY [0..15] OF CHAR;
  alg:    ARRAY [0..15] OF CHAR;
  kid:    ARRAY [0..MaxKidLen] OF CHAR;
  nB64:   ARRAY [0..1023] OF CHAR;
  eB64:   ARRAY [0..15] OF CHAR;
  nBuf:   ARRAY [0..511] OF CHAR;   (* decoded n bytes — up to 512 for 4096-bit RSA *)
  eBuf:   ARRAY [0..15] OF CHAR;    (* decoded e bytes *)
  nLen, eLen: INTEGER;
  depth:  CARDINAL;
  hasKty, hasAlg, hasKid, hasN, hasE: BOOLEAN;
  dummy:  BOOLEAN;
BEGIN
  kty[0] := 0C; alg[0] := 0C; kid[0] := 0C;
  nB64[0] := 0C; eB64[0] := 0C;
  hasKty := FALSE; hasAlg := FALSE; hasKid := FALSE;
  hasN := FALSE; hasE := FALSE;
  rsaKey := NIL;

  (* We expect to be positioned right after JObjectStart *)
  depth := 1;
  WHILE depth > 0 DO
    IF NOT Json.Next(p, tok) THEN RETURN FALSE END;

    IF tok.kind = Json.JObjectEnd THEN
      DEC(depth)
    ELSIF tok.kind = Json.JObjectStart THEN
      INC(depth);
      (* Skip nested objects *)
      IF depth > 1 THEN
        Json.Skip(p)
      END
    ELSIF tok.kind = Json.JArrayStart THEN
      (* Skip nested arrays *)
      Json.Skip(p)
    ELSIF (tok.kind = Json.JString) AND (depth = 1) THEN
      dummy := Json.GetString(p, tok, key);
      (* Read colon *)
      IF NOT Json.Next(p, tok) THEN RETURN FALSE END;
      IF tok.kind # Json.JColon THEN RETURN FALSE END;
      (* Read value *)
      IF NOT Json.Next(p, tok) THEN RETURN FALSE END;

      IF StrEq(key, "kty") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, kty);
          hasKty := TRUE
        END
      ELSIF StrEq(key, "alg") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, alg);
          hasAlg := TRUE
        END
      ELSIF StrEq(key, "kid") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, kid);
          hasKid := TRUE
        END
      ELSIF StrEq(key, "n") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, nB64);
          hasN := TRUE
        END
      ELSIF StrEq(key, "e") THEN
        IF tok.kind = Json.JString THEN
          dummy := Json.GetString(p, tok, eB64);
          hasE := TRUE
        END
      ELSE
        (* Skip unknown value *)
        IF (tok.kind = Json.JObjectStart) OR (tok.kind = Json.JArrayStart) THEN
          Json.Skip(p)
        END
      END
    ELSIF tok.kind = Json.JComma THEN
      (* skip commas between fields *)
    END
  END;

  (* Validate: must be RSA + RS256, have kid, n, e *)
  IF NOT hasKty OR NOT hasN OR NOT hasE THEN RETURN FALSE END;
  IF NOT StrEq(kty, "RSA") THEN RETURN FALSE END;
  (* Accept if alg is RS256 or absent (some JWKS don't include alg) *)
  IF hasAlg AND NOT StrEq(alg, "RS256") THEN RETURN FALSE END;
  IF NOT hasKid THEN RETURN FALSE END;

  (* Base64url-decode n and e *)
  nLen := m2_auth_b64url_decode(ADR(nB64), VAL(INTEGER, StrLen(nB64)),
                                 ADR(nBuf), 512);
  IF nLen <= 0 THEN RETURN FALSE END;

  eLen := m2_auth_b64url_decode(ADR(eB64), VAL(INTEGER, StrLen(eB64)),
                                 ADR(eBuf), 16);
  IF eLen <= 0 THEN RETURN FALSE END;

  (* Construct RSA key *)
  rsaKey := m2_oidc_rsa_from_ne(ADR(nBuf), nLen, ADR(eBuf), eLen);
  IF rsaKey = NIL THEN RETURN FALSE END;

  Assign(kid, kidOut);
  RETURN TRUE
END ParseOneKey;

PROCEDURE ParseJson(ks: KeySet;
                    json: ADDRESS; jsonLen: CARDINAL): Status;
VAR
  ksp:    KeySetPtr;
  p:      Json.Parser;
  tok:    Json.Token;
  key:    ARRAY [0..31] OF CHAR;
  kidBuf: ARRAY [0..MaxKidLen] OF CHAR;
  rsaKey: ADDRESS;
  inKeys: BOOLEAN;
  dummy:  BOOLEAN;
BEGIN
  IF ks = NIL THEN RETURN JkInvalid END;
  ksp := KeySetPtr(ks);

  Json.Init(p, json, jsonLen);

  (* Navigate to the "keys" array *)
  (* Expect: { "keys": [ ... ] } *)
  IF NOT Json.Next(p, tok) THEN RETURN JkParseFailed END;
  IF tok.kind # Json.JObjectStart THEN RETURN JkParseFailed END;

  inKeys := FALSE;
  WHILE NOT inKeys DO
    IF NOT Json.Next(p, tok) THEN RETURN JkParseFailed END;
    IF tok.kind = Json.JObjectEnd THEN RETURN JkParseFailed END;
    IF tok.kind = Json.JComma THEN
      (* skip *)
    ELSIF tok.kind = Json.JString THEN
      dummy := Json.GetString(p, tok, key);
      (* Read colon *)
      IF NOT Json.Next(p, tok) THEN RETURN JkParseFailed END;
      IF tok.kind # Json.JColon THEN RETURN JkParseFailed END;
      IF StrEq(key, "keys") THEN
        IF NOT Json.Next(p, tok) THEN RETURN JkParseFailed END;
        IF tok.kind # Json.JArrayStart THEN RETURN JkParseFailed END;
        inKeys := TRUE
      ELSE
        (* Skip non-keys value *)
        Json.Skip(p)
      END
    END
  END;

  (* Now parse array of JWK objects *)
  MutexLock(ksp^.mu);
  ClearKeys(ksp);

  LOOP
    IF NOT Json.Next(p, tok) THEN
      MutexUnlock(ksp^.mu);
      RETURN JkParseFailed
    END;
    IF tok.kind = Json.JArrayEnd THEN EXIT END;
    IF tok.kind = Json.JComma THEN
      (* skip commas between array elements *)
    ELSIF tok.kind = Json.JObjectStart THEN
      IF ParseOneKey(p, kidBuf, rsaKey) THEN
        IF ksp^.count < MaxKeys THEN
          Assign(kidBuf, ksp^.entries[ksp^.count].kid);
          ksp^.entries[ksp^.count].key := rsaKey;
          ksp^.entries[ksp^.count].valid := TRUE;
          INC(ksp^.count)
        ELSE
          (* Too many keys — free this one and skip *)
          m2_oidc_rsa_free(rsaKey)
        END
      END
    END
  END;

  MutexUnlock(ksp^.mu);
  RETURN JkOk
END ParseJson;

(* ── Key lookup ──────────────────────────────────────── *)

PROCEDURE FindKey(ks: KeySet;
                  VAR kid: ARRAY OF CHAR): Status;
VAR ksp: KeySetPtr; i: CARDINAL;
    localKid:      ARRAY [0..MaxKidLen] OF CHAR;
    localEntryKid: ARRAY [0..MaxKidLen] OF CHAR;
BEGIN
  IF ks = NIL THEN RETURN JkInvalid END;
  ksp := KeySetPtr(ks);
  Assign(kid, localKid);
  MutexLock(ksp^.mu);
  i := 0;
  WHILE i < ksp^.count DO
    IF ksp^.entries[i].valid THEN
      Assign(ksp^.entries[i].kid, localEntryKid);
      IF StrEq(localEntryKid, localKid) THEN
        MutexUnlock(ksp^.mu);
        RETURN JkOk
      END
    END;
    INC(i)
  END;
  MutexUnlock(ksp^.mu);
  RETURN JkNoSuchKid
END FindKey;

PROCEDURE Count(ks: KeySet): CARDINAL;
VAR ksp: KeySetPtr;
BEGIN
  IF ks = NIL THEN RETURN 0 END;
  ksp := KeySetPtr(ks);
  RETURN ksp^.count
END Count;

(* ── RS256 verification ──────────────────────────────── *)

PROCEDURE VerifyRS256(ks: KeySet;
                      VAR kid: ARRAY OF CHAR;
                      msg: ADDRESS; msgLen: CARDINAL;
                      sig: ADDRESS; sigLen: CARDINAL): Status;
VAR
  ksp: KeySetPtr;
  i:   CARDINAL;
  rc:  INTEGER;
  localKid:      ARRAY [0..MaxKidLen] OF CHAR;
  localEntryKid: ARRAY [0..MaxKidLen] OF CHAR;
BEGIN
  IF ks = NIL THEN RETURN JkInvalid END;
  ksp := KeySetPtr(ks);
  Assign(kid, localKid);
  MutexLock(ksp^.mu);
  i := 0;
  WHILE i < ksp^.count DO
    IF ksp^.entries[i].valid THEN
      Assign(ksp^.entries[i].kid, localEntryKid);
      IF StrEq(localEntryKid, localKid) THEN
        rc := m2_oidc_rsa_verify(ksp^.entries[i].key,
                msg, VAL(INTEGER, msgLen),
                sig, VAL(INTEGER, sigLen));
        MutexUnlock(ksp^.mu);
        IF rc = 0 THEN RETURN JkOk
        ELSE RETURN JkInvalid
        END
      END
    END;
    INC(i)
  END;
  MutexUnlock(ksp^.mu);
  RETURN JkNoSuchKid
END VerifyRS256;

END Jwks.
