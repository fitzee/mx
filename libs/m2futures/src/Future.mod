IMPLEMENTATION MODULE Future;
(* Thin delegation to the Promise module. *)

FROM SYSTEM IMPORT ADDRESS;
FROM Scheduler IMPORT Scheduler, Status;
FROM Promise IMPORT Future, Fate, Value, Error, Result,
                    ThenFn, CatchFn, VoidFn;
IMPORT Promise;

PROCEDURE Release(VAR f: Future);
BEGIN
  Promise.FutureRelease(f)
END Release;

PROCEDURE GetFate(f: Future; VAR fate: Fate): Status;
BEGIN
  RETURN Promise.GetFate(f, fate)
END GetFate;

PROCEDURE GetResultIfSettled(f: Future;
                             VAR settled: BOOLEAN;
                             VAR res: Result): Status;
BEGIN
  RETURN Promise.GetResultIfSettled(f, settled, res)
END GetResultIfSettled;

PROCEDURE FMap(s: Scheduler; f: Future;
               fn: ThenFn; user: ADDRESS;
               VAR out: Future): Status;
BEGIN
  RETURN Promise.Map(s, f, fn, user, out)
END FMap;

PROCEDURE FOnReject(s: Scheduler; f: Future;
                    fn: CatchFn; user: ADDRESS;
                    VAR out: Future): Status;
BEGIN
  RETURN Promise.OnReject(s, f, fn, user, out)
END FOnReject;

PROCEDURE FOnSettle(s: Scheduler; f: Future;
                    fn: VoidFn; user: ADDRESS;
                    VAR out: Future): Status;
BEGIN
  RETURN Promise.OnSettle(s, f, fn, user, out)
END FOnSettle;

PROCEDURE FAll(s: Scheduler; fs: ARRAY OF Future;
               VAR out: Future): Status;
BEGIN
  RETURN Promise.All(s, fs, out)
END FAll;

PROCEDURE FRace(s: Scheduler; fs: ARRAY OF Future;
                VAR out: Future): Status;
BEGIN
  RETURN Promise.Race(s, fs, out)
END FRace;

END Future.
