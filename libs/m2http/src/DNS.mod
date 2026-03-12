IMPLEMENTATION MODULE DNS;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE, BYTE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM EventLoop IMPORT Loop;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Value,
                    PromiseCreate, PromiseRelease, Resolve, Reject;
IMPORT Promise;
FROM DnsBridge IMPORT m2_dns_resolve_a, m2_dns_resolve_async;

(* ── Pending async requests ──────────────────────────── *)

(* We maintain a small fixed-size table of pending async DNS
   requests.  Each slot holds the promise and pre-allocated
   AddrRec for the callback to fill in.  The callback is
   invoked from a background pthread, so it must do minimal
   work: fill in the slot and mark it complete.  The caller
   is responsible for polling PollAsyncSlot or wiring up
   an event-loop notification. *)

CONST
  MaxPending = 16;

TYPE
  PendingSlot = RECORD
    inUse   : BOOLEAN;
    promise : Promise.Promise;
    addr    : AddrPtr;
    sched   : Scheduler;
  END;

VAR
  slots: ARRAY [0..MaxPending-1] OF PendingSlot;
  nextSlot: INTEGER;

(* ── Allocate a slot ──────────────────────────────────── *)

PROCEDURE AllocSlot(VAR id: INTEGER): BOOLEAN;
VAR i: INTEGER; idx: INTEGER;
BEGIN
  i := 0;
  WHILE i < MaxPending DO
    idx := (nextSlot + i) MOD MaxPending;
    IF NOT slots[idx].inUse THEN
      id := idx;
      slots[idx].inUse := TRUE;
      nextSlot := (idx + 1) MOD MaxPending;
      RETURN TRUE
    END;
    INC(i)
  END;
  RETURN FALSE
END AllocSlot;

(* ── Async callback (called from background thread) ──── *)

PROCEDURE AsyncCallback(callbackId: INTEGER;
                        a, b, c, d: INTEGER;
                        port: INTEGER;
                        err: INTEGER);
VAR
  ap: AddrPtr;
  v: Value;
  e: Promise.Error;
  dummy: Promise.Status;
BEGIN
  IF (callbackId < 0) OR (callbackId >= MaxPending) THEN RETURN END;
  IF NOT slots[callbackId].inUse THEN RETURN END;

  IF err < 0 THEN
    (* Resolution failed *)
    IF slots[callbackId].addr # NIL THEN
      DEALLOCATE(slots[callbackId].addr, TSIZE(AddrRec))
    END;
    e.code := 2;
    e.ptr := NIL;
    dummy := Reject(slots[callbackId].promise, e);
    PromiseRelease(slots[callbackId].promise);
    slots[callbackId].promise := NIL;
    slots[callbackId].inUse := FALSE;
    RETURN
  END;

  (* Success — fill in the AddrRec *)
  ap := slots[callbackId].addr;
  ap^.addrV4[0] := BYTE(a);
  ap^.addrV4[1] := BYTE(b);
  ap^.addrV4[2] := BYTE(c);
  ap^.addrV4[3] := BYTE(d);
  ap^.port := port;

  v.tag := 0;
  v.ptr := ap;
  dummy := Resolve(slots[callbackId].promise, v);
  PromiseRelease(slots[callbackId].promise);
  slots[callbackId].promise := NIL;
  slots[callbackId].inUse := FALSE
END AsyncCallback;

(* ── Synchronous resolve ─────────────────────────────── *)

PROCEDURE ResolveA(lp: Loop; sched: Scheduler;
                   VAR host: ARRAY OF CHAR;
                   port: INTEGER;
                   VAR outFuture: Future): Status;
VAR
  p: Promise.Promise;
  f: Future;
  pst: Promise.Status;
  ap: AddrPtr;
  rc: INTEGER;
  v: Value;
  e: Promise.Error;
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
    PromiseRelease(p); p := NIL;
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
    PromiseRelease(p); p := NIL;
    outFuture := f;
    RETURN ResolveFailed
  END;

  (* Resolve with address *)
  v.tag := 0;
  v.ptr := ap;
  dummy := Resolve(p, v);
  PromiseRelease(p); p := NIL;
  outFuture := f;
  RETURN OK
END ResolveA;

(* ── Asynchronous resolve ────────────────────────────── *)

PROCEDURE ResolveAsync(lp: Loop; sched: Scheduler;
                       VAR host: ARRAY OF CHAR;
                       port: INTEGER;
                       VAR outFuture: Future): Status;
VAR
  p: Promise.Promise;
  f: Future;
  pst: Promise.Status;
  ap: AddrPtr;
  id: INTEGER;
  e: Promise.Error;
  dummy: Promise.Status;
BEGIN
  IF sched = NIL THEN RETURN Invalid END;

  (* Allocate a pending slot *)
  IF NOT AllocSlot(id) THEN RETURN OutOfMemory END;

  (* Create promise/future pair *)
  pst := PromiseCreate(sched, p, f);
  IF pst # Promise.OK THEN
    slots[id].inUse := FALSE;
    RETURN OutOfMemory
  END;

  (* Allocate result record *)
  ALLOCATE(ap, TSIZE(AddrRec));
  IF ap = NIL THEN
    e.code := 1;
    e.ptr := NIL;
    dummy := Reject(p, e);
    PromiseRelease(p); p := NIL;
    slots[id].inUse := FALSE;
    outFuture := f;
    RETURN OutOfMemory
  END;

  (* Store in slot *)
  slots[id].promise := p;
  slots[id].addr := ap;
  slots[id].sched := sched;

  (* Launch background thread *)
  m2_dns_resolve_async(ADR(host), port, id, ADR(AsyncCallback));

  outFuture := f;
  RETURN OK
END ResolveAsync;

(* ── Module initialisation ───────────────────────────── *)

VAR ii: INTEGER;
BEGIN
  nextSlot := 0;
  ii := 0;
  WHILE ii < MaxPending DO
    slots[ii].inUse := FALSE;
    slots[ii].addr := NIL;
    INC(ii)
  END
END DNS.
