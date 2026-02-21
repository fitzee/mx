IMPLEMENTATION MODULE DNS;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE, BYTE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM EventLoop IMPORT Loop;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Promise, Value, Error,
                    PromiseCreate, Resolve, Reject;
IMPORT Promise;
FROM DnsBridge IMPORT m2_dns_resolve_a;

PROCEDURE ResolveA(lp: Loop; sched: Scheduler;
                   VAR host: ARRAY OF CHAR;
                   port: INTEGER;
                   VAR outFuture: Future): Status;
VAR
  p: Promise;
  f: Future;
  pst: Promise.Status;
  ap: AddrPtr;
  rc: INTEGER;
  v: Value;
  e: Error;
  dummy: Promise.Status;
BEGIN
  IF sched = NIL THEN RETURN Invalid END;

  (* Create promise/future pair *)
  pst := PromiseCreate(sched, p, f);
  IF pst # Promise.OK THEN RETURN OutOfMemory END;

  (* Allocate result record *)
  ALLOCATE(ap, TSIZE(AddrRec));
  IF ap = NIL THEN
    e.code := 1;
    e.ptr := NIL;
    dummy := Reject(p, e);
    outFuture := f;
    RETURN OutOfMemory
  END;

  (* Blocking DNS resolution *)
  rc := m2_dns_resolve_a(ADR(host), ADR(ap^.addrV4), ap^.port, port);

  IF rc < 0 THEN
    DEALLOCATE(ap, TSIZE(AddrRec));
    e.code := 2;
    e.ptr := NIL;
    dummy := Reject(p, e);
    outFuture := f;
    RETURN ResolveFailed
  END;

  (* Resolve with address *)
  v.tag := 0;
  v.ptr := ap;
  dummy := Resolve(p, v);
  outFuture := f;
  RETURN OK
END ResolveA;

END DNS.
