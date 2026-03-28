MODULE MainModuleTypes;
(* Adversarial test: all major type declaration kinds in main module.
   Exercises gen_type_decl_from_id for Record, Enum, Pointer-to-Record,
   Pointer, Array, ProcedureType, Set, Subrange, and Alias types
   declared directly in the program module body. *)

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST
  MaxName = 31;
  MaxItems = 7;

TYPE
  (* Record *)
  Config = RECORD
    port: CARDINAL;
    host: ARRAY [0..MaxName] OF CHAR;
    debug: BOOLEAN;
  END;

  (* Enumeration *)
  Mode = (Normal, Debug, Verbose);

  (* Pointer to record *)
  NodePtr = POINTER TO NodeRec;
  NodeRec = RECORD
    value: INTEGER;
    next: NodePtr;
  END;

  (* Array of record *)
  ItemList = ARRAY [0..MaxItems-1] OF Config;

  (* Named array of char *)
  NameBuf = ARRAY [0..MaxName] OF CHAR;

  (* Procedure type *)
  Callback = PROCEDURE(INTEGER, INTEGER): INTEGER;

  (* Subrange *)
  SmallInt = [0..255];

  (* Set type *)
  ModeSet = SET OF Mode;

  (* Pointer to char = simple pointer alias *)
  CharPtr = POINTER TO CHAR;

  (* Alias to builtin *)
  Count = CARDINAL;

VAR
  cfg: Config;
  items: ItemList;
  m: Mode;
  ms: ModeSet;
  np: NodePtr;
  node: NodeRec;
  buf: NameBuf;
  cb: Callback;
  si: SmallInt;
  cnt: Count;

PROCEDURE Add(a, b: INTEGER): INTEGER;
BEGIN
  RETURN a + b
END Add;

PROCEDURE TestConfig;
BEGIN
  cfg.port := 8080;
  cfg.host := "localhost";
  cfg.debug := TRUE;
  WriteString("port=");
  WriteInt(cfg.port, 0);
  WriteLn;
END TestConfig;

PROCEDURE TestEnum;
BEGIN
  m := Debug;
  WriteString("mode=");
  WriteInt(ORD(m), 0);
  WriteLn;
END TestEnum;

PROCEDURE TestPointerToRecord;
BEGIN
  node.value := 42;
  node.next := NIL;
  np := ADR(node);
  WriteString("node=");
  WriteInt(np^.value, 0);
  WriteLn;
END TestPointerToRecord;

PROCEDURE TestArray;
BEGIN
  items[0].port := 3000;
  items[0].host := "alpha";
  items[0].debug := FALSE;
  WriteString("item0=");
  WriteInt(items[0].port, 0);
  WriteLn;
END TestArray;

PROCEDURE TestNameBuf;
BEGIN
  buf := "hello";
  WriteString("buf=");
  WriteString(buf);
  WriteLn;
END TestNameBuf;

PROCEDURE TestCallback;
VAR r: INTEGER;
BEGIN
  cb := Add;
  r := cb(10, 25);
  WriteString("cb=");
  WriteInt(r, 0);
  WriteLn;
END TestCallback;

PROCEDURE TestSubrange;
BEGIN
  si := 200;
  WriteString("si=");
  WriteInt(si, 0);
  WriteLn;
END TestSubrange;

PROCEDURE TestSet;
BEGIN
  ms := ModeSet{Normal, Verbose};
  IF Normal IN ms THEN
    WriteString("set=ok")
  ELSE
    WriteString("set=fail")
  END;
  WriteLn;
END TestSet;

PROCEDURE TestAlias;
BEGIN
  cnt := 999;
  WriteString("cnt=");
  WriteInt(cnt, 0);
  WriteLn;
END TestAlias;

BEGIN
  TestConfig;
  TestEnum;
  TestPointerToRecord;
  TestArray;
  TestNameBuf;
  TestCallback;
  TestSubrange;
  TestSet;
  TestAlias;
END MainModuleTypes.
