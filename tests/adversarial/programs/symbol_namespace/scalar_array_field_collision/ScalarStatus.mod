IMPLEMENTATION MODULE ScalarStatus;

PROCEDURE InitResp(VAR r: Response; s: CARDINAL; l: CARDINAL);
BEGIN
  r.status := s;
  r.len := l
END InitResp;

PROCEDURE GetStatus(VAR r: Response): CARDINAL;
BEGIN
  RETURN r.status
END GetStatus;

END ScalarStatus.
