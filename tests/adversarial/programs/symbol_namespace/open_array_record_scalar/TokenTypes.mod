IMPLEMENTATION MODULE TokenTypes;

FROM Strings IMPORT Assign;

PROCEDURE InitToken(VAR t: TokenRecord; n: ARRAY OF CHAR; s: CARDINAL);
BEGIN
  Assign(n, t.name);
  t.role := 0;
  t.createdAt := 0;
  t.expiresAt := 0;
  t.lastUsedAt := 0;
  t.status := s
END InitToken;

END TokenTypes.
