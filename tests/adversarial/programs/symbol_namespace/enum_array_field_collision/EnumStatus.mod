IMPLEMENTATION MODULE EnumStatus;

FROM Strings IMPORT Assign;

PROCEDURE InitSpan(VAR s: Span; nm: ARRAY OF CHAR; st: SpanStatus; dur: CARDINAL);
BEGIN
  Assign(nm, s.name);
  s.status := st;
  s.duration := dur
END InitSpan;

END EnumStatus.
