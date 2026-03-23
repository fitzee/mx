MODULE StringConstOpenArray;
(* Regression: string constants passed to ARRAY OF CHAR (open array) params
   were emitted as ptr-to-ptr with HIGH=0 instead of the string data with
   correct HIGH. The sema knows these are ConstValue::String — the codegen
   must use the interned literal directly. *)

FROM InOut IMPORT WriteString, WriteLn;

CONST
  Greeting = "hello";
  World    = " world";
  Long     = "abcdefghijklmnopqrstuvwxyz";

BEGIN
  (* Pass string const directly to WriteString (open array) *)
  WriteString(Greeting); WriteLn;
  WriteString(World); WriteLn;
  WriteString(Long); WriteLn;
  WriteString("done"); WriteLn
END StringConstOpenArray.
