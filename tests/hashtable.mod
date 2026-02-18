MODULE HashTable;
(* Simple hash table implementation using separate chaining *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

CONST
  TableSize = 16;
  Empty = "";

TYPE
  EntryPtr = POINTER TO Entry;
  Entry = RECORD
    key: ARRAY [0..31] OF CHAR;
    value: INTEGER;
    next: EntryPtr;
  END;

  Table = ARRAY [0..TableSize-1] OF EntryPtr;

VAR
  ht: Table;
  i: INTEGER;

PROCEDURE Hash(key: ARRAY OF CHAR): INTEGER;
  VAR h, i: INTEGER;
BEGIN
  h := 0;
  FOR i := 0 TO HIGH(key) DO
    IF key[i] = 0C THEN RETURN h MOD TableSize END;
    h := h * 31 + ORD(key[i])
  END;
  RETURN h MOD TableSize
END Hash;

PROCEDURE Init(VAR t: Table);
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO TableSize - 1 DO
    t[i] := NIL
  END
END Init;

PROCEDURE StrEq(a, b: ARRAY OF CHAR): BOOLEAN;
  VAR i: INTEGER;
BEGIN
  i := 0;
  LOOP
    IF (i > HIGH(a)) OR (i > HIGH(b)) THEN RETURN TRUE END;
    IF a[i] # b[i] THEN RETURN FALSE END;
    IF a[i] = 0C THEN RETURN TRUE END;
    INC(i)
  END
END StrEq;

PROCEDURE Put(VAR t: Table; key: ARRAY OF CHAR; value: INTEGER);
  VAR h: INTEGER;
      p: EntryPtr;
BEGIN
  h := Hash(key);
  (* Check if key already exists *)
  p := t[h];
  WHILE p # NIL DO
    IF StrEq(p^.key, key) THEN
      p^.value := value;
      RETURN
    END;
    p := p^.next
  END;
  (* Add new entry *)
  NEW(p);
  p^.key := key;
  p^.value := value;
  p^.next := t[h];
  t[h] := p
END Put;

PROCEDURE Get(t: Table; key: ARRAY OF CHAR; VAR value: INTEGER): BOOLEAN;
  VAR h: INTEGER;
      p: EntryPtr;
BEGIN
  h := Hash(key);
  p := t[h];
  WHILE p # NIL DO
    IF StrEq(p^.key, key) THEN
      value := p^.value;
      RETURN TRUE
    END;
    p := p^.next
  END;
  RETURN FALSE
END Get;

PROCEDURE Contains(t: Table; key: ARRAY OF CHAR): BOOLEAN;
  VAR dummy: INTEGER;
BEGIN
  RETURN Get(t, key, dummy)
END Contains;

PROCEDURE Count(t: Table): INTEGER;
  VAR i, n: INTEGER;
      p: EntryPtr;
BEGIN
  n := 0;
  FOR i := 0 TO TableSize - 1 DO
    p := t[i];
    WHILE p # NIL DO
      INC(n);
      p := p^.next
    END
  END;
  RETURN n
END Count;

PROCEDURE FreeTable(VAR t: Table);
  VAR i: INTEGER;
      p, tmp: EntryPtr;
BEGIN
  FOR i := 0 TO TableSize - 1 DO
    p := t[i];
    WHILE p # NIL DO
      tmp := p;
      p := p^.next;
      DISPOSE(tmp)
    END;
    t[i] := NIL
  END
END FreeTable;

VAR
  val: INTEGER;
  found: BOOLEAN;

BEGIN
  Init(ht);

  (* Insert some key-value pairs *)
  Put(ht, "alpha", 1);
  Put(ht, "beta", 2);
  Put(ht, "gamma", 3);
  Put(ht, "delta", 4);
  Put(ht, "epsilon", 5);

  WriteString("Count: "); WriteInt(Count(ht), 1); WriteLn;

  (* Look up values *)
  found := Get(ht, "beta", val);
  WriteString("beta = ");
  IF found THEN WriteInt(val, 1) ELSE WriteString("not found") END;
  WriteLn;

  found := Get(ht, "delta", val);
  WriteString("delta = ");
  IF found THEN WriteInt(val, 1) ELSE WriteString("not found") END;
  WriteLn;

  found := Get(ht, "omega", val);
  WriteString("omega = ");
  IF found THEN WriteInt(val, 1) ELSE WriteString("not found") END;
  WriteLn;

  (* Update existing key *)
  Put(ht, "beta", 22);
  found := Get(ht, "beta", val);
  WriteString("beta (updated) = ");
  IF found THEN WriteInt(val, 1) ELSE WriteString("not found") END;
  WriteLn;

  WriteString("Contains alpha: ");
  IF Contains(ht, "alpha") THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  WriteString("Contains zeta: ");
  IF Contains(ht, "zeta") THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  FreeTable(ht);
  WriteString("After free, count: "); WriteInt(Count(ht), 1); WriteLn;

  WriteString("Done"); WriteLn
END HashTable.
