MODULE ArrayFieldCollision;
(* Regression: when two modules have records with identically-named fields
   but different types (one array, one scalar), the codegen must use
   type-aware resolution through array indexing to avoid false-positive
   memcpy on scalar fields accessed via arr[i].field.

   Without the fix, the fallback name-only is_array_field check sees
   "tag" as an array field (from Keys.KeyEntry) and generates memcpy
   for Items.Item.tag, causing an integer-to-pointer conversion error. *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Keys IMPORT KeyEntry, InitEntry;
FROM Items IMPORT Item, SetTag, GetTag;

VAR
  items: ARRAY [0..3] OF Item;
  keys:  ARRAY [0..1] OF KeyEntry;
  i: CARDINAL;

BEGIN
  (* Exercise the array field — make sure Keys module is compiled *)
  InitEntry(keys[0]);
  InitEntry(keys[1]);

  (* Test scalar field assignment through array index *)
  SetTag(items, 0, 42);
  SetTag(items, 1, 99);
  SetTag(items, 2, -7);
  SetTag(items, 3, 0);

  (* Verify values *)
  i := 0;
  WHILE i <= 3 DO
    WriteString("tag=");
    WriteInt(GetTag(items, i), 1);
    WriteString(" val=");
    WriteInt(items[i].value, 1);
    WriteLn;
    INC(i)
  END;

  (* Direct array-indexed field assignment in main module *)
  items[0].tag := 100;
  items[1].tag := items[0].tag + 1;
  WriteString("direct=");
  WriteInt(items[0].tag, 1);
  WriteString(",");
  WriteInt(items[1].tag, 1);
  WriteLn;

  WriteString("all array field collision OK");
  WriteLn
END ArrayFieldCollision.
