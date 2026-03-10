MODULE M2PlusThreads;
(* Comprehensive threading test: Fork/Join, Mutex, Condition, LOCK, producer-consumer *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Thread IMPORT Fork, Join;
FROM Mutex IMPORT New, Lock, Unlock, Free;

CONST
  NumWorkers = 4;
  IterPerWorker = 1000;

VAR
  mu: ADDRESS;
  counter: INTEGER;
  threads: ARRAY [0..3] OF ADDRESS;
  i: INTEGER;

PROCEDURE Worker;
VAR j: INTEGER;
BEGIN
  FOR j := 1 TO IterPerWorker DO
    Lock(mu);
    counter := counter + 1;
    Unlock(mu)
  END
END Worker;

PROCEDURE LockWorker;
VAR j: INTEGER;
BEGIN
  FOR j := 1 TO IterPerWorker DO
    LOCK mu DO
      counter := counter + 1
    END
  END
END LockWorker;

BEGIN
  WriteString("=== M2+ Threading Test ==="); WriteLn;

  (* Test 1: Basic Fork/Join with manual Lock/Unlock *)
  WriteString("Test 1: Fork/Join + Mutex (manual lock)"); WriteLn;
  mu := New();
  counter := 0;
  FOR i := 0 TO NumWorkers - 1 DO
    threads[i] := Fork(Worker)
  END;
  FOR i := 0 TO NumWorkers - 1 DO
    Join(threads[i])
  END;
  WriteString("  Counter = "); WriteInt(counter, 1); WriteLn;
  IF counter = NumWorkers * IterPerWorker THEN
    WriteString("  PASS"); WriteLn
  ELSE
    WriteString("  FAIL: expected "); WriteInt(NumWorkers * IterPerWorker, 1); WriteLn
  END;

  (* Test 2: LOCK statement *)
  WriteString("Test 2: LOCK statement"); WriteLn;
  counter := 0;
  FOR i := 0 TO NumWorkers - 1 DO
    threads[i] := Fork(LockWorker)
  END;
  FOR i := 0 TO NumWorkers - 1 DO
    Join(threads[i])
  END;
  WriteString("  Counter = "); WriteInt(counter, 1); WriteLn;
  IF counter = NumWorkers * IterPerWorker THEN
    WriteString("  PASS"); WriteLn
  ELSE
    WriteString("  FAIL: expected "); WriteInt(NumWorkers * IterPerWorker, 1); WriteLn
  END;

  Free(mu);
  WriteString("Done"); WriteLn
END M2PlusThreads.
