IMPLEMENTATION MODULE FsmTrace;

FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM Fsm IMPORT StateId, EventId, ActionId, StepStatus,
                Ok, NoTransition, GuardRejected, Error;

PROCEDURE ConsoleTrace(traceCtx: ADDRESS;
                       fromState, toState: StateId;
                       ev: EventId; action: ActionId;
                       status: StepStatus);
BEGIN
  WriteString("FSM: ");
  WriteCard(fromState, 0);
  WriteString(" -> ");
  WriteCard(toState, 0);
  WriteString(" ev=");
  WriteCard(ev, 0);
  WriteString(" act=");
  WriteCard(action, 0);
  WriteString(" ");
  IF status = Ok THEN
    WriteString("OK")
  ELSIF status = NoTransition THEN
    WriteString("NO_TRANS")
  ELSIF status = GuardRejected THEN
    WriteString("REJECTED")
  ELSIF status = Error THEN
    WriteString("ERROR")
  END;
  WriteLn
END ConsoleTrace;

END FsmTrace.
