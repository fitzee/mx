IMPLEMENTATION MODULE AddrKeyLib;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Storage IMPORT ALLOCATE;

TYPE
  Entry = RECORD
    id:  INTEGER;
    ptr: ADDRESS;
  END;
  EntryPtr = POINTER TO Entry;

VAR
  store: Entry;

PROCEDURE GetHandle(id: INTEGER; VAR key: ADDRESS): BOOLEAN;
BEGIN
  IF id = store.id THEN
    key := store.ptr;
    RETURN TRUE
  END;
  key := NIL;
  RETURN FALSE
END GetHandle;

BEGIN
  store.id := 42;
  store.ptr := ADR(store)
END AddrKeyLib.
