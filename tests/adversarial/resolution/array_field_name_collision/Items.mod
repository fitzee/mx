IMPLEMENTATION MODULE Items;

VAR
  tagSource: ARRAY [0..7] OF INTEGER;

PROCEDURE SetTag(VAR items: ARRAY OF Item; idx: CARDINAL; t: INTEGER);
BEGIN
  (* arr[i].field := scalar — must not memcpy *)
  items[idx].tag := t;
  items[idx].value := t * 10
END SetTag;

PROCEDURE GetTag(VAR items: ARRAY OF Item; idx: CARDINAL): INTEGER;
BEGIN
  (* Also test reading through arr[i].field *)
  RETURN items[idx].tag
END GetTag;

BEGIN
  (* Init source array so we can test arr[i] := arr[j] patterns *)
  tagSource[0] := 42;
  tagSource[1] := 99;
  tagSource[2] := -7;
  tagSource[3] := 0
END Items.
