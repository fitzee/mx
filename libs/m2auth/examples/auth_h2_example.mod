MODULE AuthH2Example;
(* Example: HTTP/2 server with authentication middleware.

   Demonstrates:
   1. Creating a keyring with an HS256 key
   2. Configuring a verifier and policy
   3. Adding AuthMw as Http2Server middleware
   4. Handler accessing the authenticated Principal

   To test:
     1. Build and run this server
     2. Generate a token using SignToken
     3. Send an HTTP/2 request with Authorization: Bearer <token>

   Note: This is a compile-check example.  A real deployment
   would load keys from configuration and use proper TLS certs. *)

FROM InOut IMPORT WriteString, WriteLn;
FROM SYSTEM IMPORT ADDRESS;
FROM Auth IMPORT Status, OK, TokenKind, JwtHS256,
                  Keyring, Verifier, Policy, Principal, SymKey,
                  KeyringCreate, KeyringAddHS256, KeyringDestroy,
                  VerifierCreate, VerifierDestroy,
                  PolicyCreate, PolicyAllowScope, PolicyDestroy,
                  SignToken, InitPrincipal;
FROM AuthMiddleware IMPORT Configure, AuthMw, GetPrincipal;
FROM AuthBridge IMPORT m2_auth_init, m2_auth_get_unix_time;

VAR
  kr: Keyring;
  v: Verifier;
  pol: Policy;
  p: Principal;
  st: Status;
  key: SymKey;
  token: ARRAY [0..2047] OF CHAR;
  tokenLen: CARDINAL;
  now: LONGINT;
  i: CARDINAL;

BEGIN
  m2_auth_init;

  (* 1. Create keyring and add a signing key *)
  st := KeyringCreate(kr);
  IF st # OK THEN
    WriteString("Failed to create keyring"); WriteLn; HALT
  END;

  FOR i := 0 TO 31 DO
    key[i] := 'X'
  END;
  st := KeyringAddHS256(kr, "demo-key", key);

  (* 2. Create verifier *)
  st := VerifierCreate(v, kr);

  (* 3. Create policy: allow "read" scope *)
  st := PolicyCreate(pol);
  st := PolicyAllowScope(pol, "read");

  (* 4. Configure middleware *)
  Configure(v, pol);

  (* 5. Generate a demo token *)
  InitPrincipal(p);
  p.subject[0] := 'd'; p.subject[1] := 'e'; p.subject[2] := 'm';
  p.subject[3] := 'o'; p.subject[4] := 0C;
  now := m2_auth_get_unix_time();
  p.iatUnix := now;
  p.expUnix := now + 3600;
  p.scopeCount := 1;
  p.scopes[0][0] := 'r'; p.scopes[0][1] := 'e';
  p.scopes[0][2] := 'a'; p.scopes[0][3] := 'd';
  p.scopes[0][4] := 0C;

  st := SignToken(kr, JwtHS256, "demo-key", p, token, tokenLen);
  IF st = OK THEN
    WriteString("Demo token generated ("); WriteString("len=");
    (* WriteCard(tokenLen, 0); *)
    WriteString("):"); WriteLn;
    WriteString(token); WriteLn
  ELSE
    WriteString("Failed to generate token"); WriteLn
  END;

  (* In a real server, you would:
     AddMiddleware(server, AuthMw, NIL);
     AddRoute(server, "GET", "/api/data", DataHandler, NIL);
     Start(server);
  *)

  WriteString("AuthMw configured, ready for Http2Server.AddMiddleware");
  WriteLn;

  (* Cleanup *)
  st := PolicyDestroy(pol);
  st := VerifierDestroy(v);
  st := KeyringDestroy(kr)
END AuthH2Example.
