IMPLEMENTATION MODULE RpcErrors;

FROM Strings IMPORT Assign;

PROCEDURE ToString(code: CARDINAL; VAR s: ARRAY OF CHAR);
BEGIN
  IF code = Ok THEN
    Assign("Ok", s)
  ELSIF code = BadRequest THEN
    Assign("BadRequest", s)
  ELSIF code = UnknownMethod THEN
    Assign("UnknownMethod", s)
  ELSIF code = Timeout THEN
    Assign("Timeout", s)
  ELSIF code = Internal THEN
    Assign("Internal", s)
  ELSIF code = TooLarge THEN
    Assign("TooLarge", s)
  ELSIF code = Closed THEN
    Assign("Closed", s)
  ELSE
    Assign("Unknown", s)
  END
END ToString;

END RpcErrors.
