IMPLEMENTATION MODULE AuthMiddleware;

  (* HTTP/2 auth middleware and RPC auth guard implementation. *)

  FROM SYSTEM IMPORT ADDRESS, ADR;
  FROM Strings IMPORT Assign, Length;
  FROM Auth IMPORT Verifier, Policy, Principal, Status,
                    OK, Invalid, Denied, BadSignature,
                    Expired, NotYetValid, VerifyFailed,
                    VerifyBearerToken, Authorize, InitPrincipal;
  FROM Http2ServerTypes IMPORT Request, Response, MaxReqHeaders,
                                MaxReqValueLen;

  VAR
    gVerifier:  Verifier;
    gPolicy:    Policy;
    gPrincipal: Principal;
    gHasPrincipal: BOOLEAN;
    gAuditProc: AuditProc;
    gAuditCtx:  ADDRESS;
    gHasAudit:  BOOLEAN;
    gInnerHandler: PROCEDURE(ADDRESS, CARDINAL,
                              ADDRESS, CARDINAL,
                              ADDRESS,
                              VAR ADDRESS,
                              VAR CARDINAL,
                              VAR BOOLEAN);
    gHasInner: BOOLEAN;

  (* ── String helpers ──────────────────────────────── *)

  PROCEDURE StrEq(VAR a, b: ARRAY OF CHAR): BOOLEAN;
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
  END StrEq;

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

  PROCEDURE ToLower(ch: CHAR): CHAR;
  BEGIN
    IF (ch >= 'A') AND (ch <= 'Z') THEN
      RETURN CHR(ORD(ch) + 32)
    END;
    RETURN ch
  END ToLower;

  PROCEDURE StrEqCI(VAR a, b: ARRAY OF CHAR): BOOLEAN;
  VAR i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(a)) AND (i <= HIGH(b)) AND
          (a[i] # 0C) AND (ToLower(a[i]) = ToLower(b[i])) DO
      INC(i)
    END;
    IF (i > HIGH(a)) OR (a[i] = 0C) THEN
      IF (i > HIGH(b)) OR (b[i] = 0C) THEN RETURN TRUE END
    END;
    RETURN FALSE
  END StrEqCI;

  (* ── Configuration ───────────────────────────────── *)

  PROCEDURE Configure(v: Verifier; pol: Policy);
  BEGIN
    gVerifier := v;
    gPolicy := pol
  END Configure;

  PROCEDURE SetAuditCallback(proc: AuditProc; ctx: ADDRESS);
  BEGIN
    gAuditProc := proc;
    gAuditCtx := ctx;
    gHasAudit := TRUE
  END SetAuditCallback;

  (* ── Audit helper ────────────────────────────────── *)

  PROCEDURE EmitAudit(kind: AuditEventKind;
                      VAR subj: ARRAY OF CHAR;
                      VAR meth: ARRAY OF CHAR;
                      VAR path: ARRAY OF CHAR;
                      st: Status;
                      VAR remote: ARRAY OF CHAR);
  VAR ev: AuditEvent;
  BEGIN
    IF NOT gHasAudit THEN RETURN END;
    ev.kind := kind;
    StrCopy(subj, ev.subject);
    StrCopy(meth, ev.method);
    StrCopy(path, ev.path);
    ev.status := st;
    StrCopy(remote, ev.remoteAddr);
    gAuditProc(ev, gAuditCtx)
  END EmitAudit;

  (* ── Extract Bearer token from Authorization header ─ *)

  PROCEDURE FindBearerToken(VAR req: Request;
                            VAR token: ARRAY OF CHAR;
                            VAR tokenLen: CARDINAL): BOOLEAN;
  VAR
    i, j: CARDINAL;
    authName: ARRAY [0..12] OF CHAR;
  BEGIN
    Assign("authorization", authName);
    tokenLen := 0;
    FOR i := 0 TO req.numHeaders - 1 DO
      IF StrEqCI(req.headers[i].name, authName) THEN
        (* Check "Bearer " prefix (7 chars) *)
        IF req.headers[i].valLen > 7 THEN
          IF (req.headers[i].value[0] = 'B') AND
             (req.headers[i].value[1] = 'e') AND
             (req.headers[i].value[2] = 'a') AND
             (req.headers[i].value[3] = 'r') AND
             (req.headers[i].value[4] = 'e') AND
             (req.headers[i].value[5] = 'r') AND
             (req.headers[i].value[6] = ' ') THEN
            j := 7;
            WHILE (j < req.headers[i].valLen) AND
                  (tokenLen <= HIGH(token)) DO
              token[tokenLen] := req.headers[i].value[j];
              INC(tokenLen); INC(j)
            END;
            IF tokenLen <= HIGH(token) THEN
              token[tokenLen] := 0C
            END;
            RETURN TRUE
          END
        END
      END
    END;
    RETURN FALSE
  END FindBearerToken;

  (* ── Set response status ─────────────────────────── *)

  PROCEDURE SetStatus(VAR resp: Response; code: CARDINAL);
  BEGIN
    resp.status := code
  END SetStatus;

  (* ── HTTP/2 Middleware ───────────────────────────── *)

  PROCEDURE AuthMw(VAR req: Request;
                   VAR resp: Response;
                   ctx: ADDRESS): BOOLEAN;
  VAR
    token: ARRAY [0..MaxReqValueLen] OF CHAR;
    tokenLen: CARDINAL;
    st: Status;
    empty: ARRAY [0..0] OF CHAR;
  BEGIN
    gHasPrincipal := FALSE;
    empty[0] := 0C;

    (* Extract token *)
    IF NOT FindBearerToken(req, token, tokenLen) THEN
      EmitAudit(AeAuthFail, empty, req.method, req.path,
                Invalid, req.remoteAddr);
      SetStatus(resp, 401);
      RETURN FALSE
    END;

    (* Verify *)
    st := VerifyBearerToken(gVerifier, token, gPrincipal);
    IF st # OK THEN
      EmitAudit(AeAuthFail, empty, req.method, req.path,
                st, req.remoteAddr);
      IF (st = Expired) OR (st = NotYetValid) THEN
        SetStatus(resp, 401)
      ELSIF st = BadSignature THEN
        SetStatus(resp, 401)
      ELSE
        SetStatus(resp, 401)
      END;
      RETURN FALSE
    END;

    gHasPrincipal := TRUE;

    (* Authorize if policy is set *)
    IF gPolicy # NIL THEN
      st := Authorize(gPolicy, gPrincipal);
      IF st # OK THEN
        EmitAudit(AeAuthzDeny, gPrincipal.subject,
                  req.method, req.path, st, req.remoteAddr);
        SetStatus(resp, 403);
        RETURN FALSE
      END
    END;

    EmitAudit(AeAuthOK, gPrincipal.subject,
              req.method, req.path, OK, req.remoteAddr);
    RETURN TRUE
  END AuthMw;

  (* ── GetPrincipal ────────────────────────────────── *)

  PROCEDURE GetPrincipal(VAR p: Principal): BOOLEAN;
  BEGIN
    IF gHasPrincipal THEN
      p := gPrincipal;
      RETURN TRUE
    END;
    RETURN FALSE
  END GetPrincipal;

  (* ── RPC Auth Guard ──────────────────────────────── *)

  PROCEDURE SetRpcGuardHandler(
    innerHandler: PROCEDURE(ADDRESS, CARDINAL,
                            ADDRESS, CARDINAL,
                            ADDRESS,
                            VAR ADDRESS,
                            VAR CARDINAL,
                            VAR BOOLEAN));
  BEGIN
    gInnerHandler := innerHandler;
    gHasInner := TRUE
  END SetRpcGuardHandler;

  PROCEDURE RpcAuthGuard(ctx: ADDRESS;
                          reqId: CARDINAL;
                          methodPtr: ADDRESS;
                          methodLen: CARDINAL;
                          body: ADDRESS;
                          VAR outBody: ADDRESS;
                          VAR errCode: CARDINAL;
                          VAR ok: BOOLEAN);
  VAR
    st: Status;
    token: ARRAY [0..MaxReqValueLen] OF CHAR;
  BEGIN
    ok := FALSE;
    errCode := 401;

    IF NOT gHasInner THEN RETURN END;

    (* For RPC guard, the token extraction depends on the transport.
       When used over Http2Server, the middleware handles it.
       For standalone RPC, the caller pre-populates the token
       in a transport-specific way.  This guard just verifies
       the principal set by the middleware. *)
    IF NOT gHasPrincipal THEN
      RETURN
    END;

    (* Authorize *)
    IF gPolicy # NIL THEN
      st := Authorize(gPolicy, gPrincipal);
      IF st # OK THEN
        errCode := 403;
        RETURN
      END
    END;

    (* Delegate to inner handler *)
    gInnerHandler(ctx, reqId, methodPtr, methodLen,
                  body, outBody, errCode, ok)
  END RpcAuthGuard;

BEGIN
  gVerifier := NIL;
  gPolicy := NIL;
  gHasPrincipal := FALSE;
  gHasAudit := FALSE;
  gHasInner := FALSE;
  InitPrincipal(gPrincipal)
END AuthMiddleware.
