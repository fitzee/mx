MODULE StrSafe;
(* Regression tests for bounded string operations.
   All destination arrays are deliberately small to exercise truncation. *)

FROM InOut IMPORT WriteString, WriteLn;
FROM Strings IMPORT Assign, Concat, Insert, Copy, Delete;

VAR
  tiny: ARRAY [0..7] OF CHAR;    (* capacity 8: indices 0..7 *)
  med:  ARRAY [0..15] OF CHAR;   (* capacity 16 *)
  big:  ARRAY [0..63] OF CHAR;

BEGIN
  (* Test 1: Assign truncation *)
  (* "Hello, World!" is 13 chars, tiny holds max 7 chars + NUL *)
  Assign("Hello, World!", tiny);
  WriteString("1:"); WriteString(tiny); WriteLn;

  (* Assign that fits *)
  Assign("Hi", tiny);
  WriteString("2:"); WriteString(tiny); WriteLn;

  (* Test 2: Concat truncation *)
  (* "ABCD" + "EFGHIJK" = 11 chars, tiny holds 7 *)
  Concat("ABCD", "EFGHIJK", tiny);
  WriteString("3:"); WriteString(tiny); WriteLn;

  (* Concat where first arg alone overflows *)
  Concat("ABCDEFGHIJ", "XY", tiny);
  WriteString("4:"); WriteString(tiny); WriteLn;

  (* Concat that fits *)
  Concat("AB", "CD", tiny);
  WriteString("5:"); WriteString(tiny); WriteLn;

  (* Test 3: Insert near end truncation *)
  (* Start with "ABCDEF" (6 chars), insert "XYZ" at pos 4 *)
  (* Without truncation: "ABCDXYZEF" = 9, but tiny holds 7 *)
  Assign("ABCDEF", tiny);
  Insert("XYZ", tiny, 4);
  WriteString("6:"); WriteString(tiny); WriteLn;

  (* Insert at beginning with overflow *)
  Assign("ABCDEF", tiny);
  Insert("1234", tiny, 0);
  WriteString("7:"); WriteString(tiny); WriteLn;

  (* Test 4: Copy truncation *)
  Assign("Hello, World!", big);
  Copy(big, 0, 10, tiny);
  WriteString("8:"); WriteString(tiny); WriteLn;

  (* Copy that fits *)
  Copy(big, 7, 5, med);
  WriteString("9:"); WriteString(med); WriteLn;

  (* Test 5: Delete *)
  Assign("ABCDEFGH", med);
  Delete(med, 2, 3);
  WriteString("10:"); WriteString(med); WriteLn;

  WriteString("Done"); WriteLn
END StrSafe.
