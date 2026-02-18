MODULE Sieve;
(* Sieve of Eratosthenes *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST Max = 1000;

VAR
  isPrime: ARRAY [2..Max] OF BOOLEAN;
  i, j, count: INTEGER;
  lastPrime: INTEGER;

BEGIN
  (* Initialize all to TRUE *)
  FOR i := 2 TO Max DO
    isPrime[i] := TRUE
  END;

  (* Sieve *)
  FOR i := 2 TO Max DO
    IF isPrime[i] THEN
      j := i * i;
      WHILE j <= Max DO
        isPrime[j] := FALSE;
        j := j + i
      END
    END
  END;

  (* Count and find last prime *)
  count := 0;
  lastPrime := 0;
  FOR i := 2 TO Max DO
    IF isPrime[i] THEN
      INC(count);
      lastPrime := i
    END
  END;

  WriteString("Primes up to "); WriteInt(Max, 1); WriteLn;
  WriteString("Count: "); WriteInt(count, 1); WriteLn;
  WriteString("Last prime: "); WriteInt(lastPrime, 1); WriteLn;

  (* Print first 25 primes *)
  WriteString("First 25: ");
  count := 0;
  FOR i := 2 TO Max DO
    IF isPrime[i] THEN
      WriteInt(i, 5);
      INC(count);
      IF count >= 25 THEN i := Max END (* break *)
    END
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END Sieve.
