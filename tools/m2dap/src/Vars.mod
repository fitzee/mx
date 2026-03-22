IMPLEMENTATION MODULE Vars;

CONST
  MaxRefs = 512;

TYPE
  RefKind = (RScope, RVar);

  RefEntry = RECORD
    kind: RefKind;
    frame: INTEGER;      (* scope: frame index *)
    scope: INTEGER;      (* scope: ScopeLocals/Args/Globals *)
    parent: INTEGER;     (* var: parent reference *)
    child: INTEGER;      (* var: child index *)
  END;

VAR
  refs: ARRAY [0..MaxRefs-1] OF RefEntry;
  count: INTEGER;

PROCEDURE Reset;
BEGIN
  count := 0
END Reset;

PROCEDURE AllocScopeRef(frameIdx: INTEGER;
                        scopeKind: INTEGER): INTEGER;
VAR id: INTEGER;
BEGIN
  IF count >= MaxRefs THEN RETURN 0 END;
  id := count + 1;  (* DAP refs must be > 0 *)
  refs[count].kind := RScope;
  refs[count].frame := frameIdx;
  refs[count].scope := scopeKind;
  refs[count].parent := 0;
  refs[count].child := 0;
  INC(count);
  RETURN id
END AllocScopeRef;

PROCEDURE AllocVarRef(parentRef: INTEGER;
                      childIdx: INTEGER): INTEGER;
VAR id: INTEGER;
BEGIN
  IF count >= MaxRefs THEN RETURN 0 END;
  id := count + 1;
  refs[count].kind := RVar;
  refs[count].frame := 0;
  refs[count].scope := 0;
  refs[count].parent := parentRef;
  refs[count].child := childIdx;
  INC(count);
  RETURN id
END AllocVarRef;

PROCEDURE GetRefInfo(ref: INTEGER;
                     VAR frameIdx: INTEGER;
                     VAR scopeKind: INTEGER;
                     VAR parentRef: INTEGER;
                     VAR childIdx: INTEGER;
                     VAR isScope: BOOLEAN);
VAR idx: INTEGER;
BEGIN
  idx := ref - 1;
  IF (idx < 0) OR (idx >= count) THEN
    frameIdx := 0;
    scopeKind := 0;
    parentRef := 0;
    childIdx := 0;
    isScope := TRUE;
    RETURN
  END;
  isScope := (refs[idx].kind = RScope);
  frameIdx := refs[idx].frame;
  scopeKind := refs[idx].scope;
  parentRef := refs[idx].parent;
  childIdx := refs[idx].child
END GetRefInfo;

BEGIN
  count := 0
END Vars.
