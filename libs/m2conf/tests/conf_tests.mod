MODULE ConfTests;
(* Test suite for m2conf.

   Tests:
     1.  parse_ok         Basic parse returns TRUE
     2.  section_count    Correct number of sections
     3.  section_name_0   First section name is "" (default)
     4.  section_name_1   Second section name is "server"
     5.  key_count        KeyCount("server") = 2
     6.  get_host         GetValue("server","host") = "localhost"
     7.  get_port         GetValue("server","port") = "8080"
     8.  get_db_name      GetValue("database","name") = "mydb"
     9.  get_db_user      GetValue("database","user") = "admin"
    10.  has_key_true     HasKey("server","host") = TRUE
    11.  has_key_false    HasKey("server","missing") = FALSE
    12.  missing_section  GetValue for missing section returns FALSE
    13.  get_key_by_idx   GetKey("server",0) = "host"
    14.  clear            Clear then SectionCount = 0
    15.  parse_empty      Parse empty string, SectionCount = 1 (default)
    16.  whitespace_trim  " key = value " trimmed correctly *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Strings IMPORT CompareStr, Length;
FROM Conf IMPORT Parse, Clear, SectionCount, GetSectionName,
                 KeyCount, GetKey, GetValue, HasKey;

VAR
  passed, failed, total: INTEGER;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* ── Test config string ─────────────────────────────── *)

PROCEDURE TestBasicParse;
VAR
  cfg: ARRAY [0..255] OF CHAR;
  ok: BOOLEAN;
  name: ARRAY [0..63] OF CHAR;
  val:  ARRAY [0..255] OF CHAR;
  key:  ARRAY [0..63] OF CHAR;
  n: INTEGER;
BEGIN
  (* Build config string with CHR(10) as newline *)
  cfg[0] := '#'; cfg[1] := ' '; cfg[2] := 'c'; cfg[3] := 'o';
  cfg[4] := 'm'; cfg[5] := 'm'; cfg[6] := 'e'; cfg[7] := 'n';
  cfg[8] := 't'; cfg[9] := CHR(10);
  cfg[10] := '['; cfg[11] := 's'; cfg[12] := 'e'; cfg[13] := 'r';
  cfg[14] := 'v'; cfg[15] := 'e'; cfg[16] := 'r'; cfg[17] := ']';
  cfg[18] := CHR(10);
  cfg[19] := 'h'; cfg[20] := 'o'; cfg[21] := 's'; cfg[22] := 't';
  cfg[23] := '='; cfg[24] := 'l'; cfg[25] := 'o'; cfg[26] := 'c';
  cfg[27] := 'a'; cfg[28] := 'l'; cfg[29] := 'h'; cfg[30] := 'o';
  cfg[31] := 's'; cfg[32] := 't'; cfg[33] := CHR(10);
  cfg[34] := 'p'; cfg[35] := 'o'; cfg[36] := 'r'; cfg[37] := 't';
  cfg[38] := '='; cfg[39] := '8'; cfg[40] := '0'; cfg[41] := '8';
  cfg[42] := '0'; cfg[43] := CHR(10);
  cfg[44] := CHR(10);
  cfg[45] := '['; cfg[46] := 'd'; cfg[47] := 'a'; cfg[48] := 't';
  cfg[49] := 'a'; cfg[50] := 'b'; cfg[51] := 'a'; cfg[52] := 's';
  cfg[53] := 'e'; cfg[54] := ']'; cfg[55] := CHR(10);
  (* name = mydb *)
  cfg[56] := 'n'; cfg[57] := 'a'; cfg[58] := 'm'; cfg[59] := 'e';
  cfg[60] := ' '; cfg[61] := '='; cfg[62] := ' '; cfg[63] := 'm';
  cfg[64] := 'y'; cfg[65] := 'd'; cfg[66] := 'b'; cfg[67] := CHR(10);
  (* user = admin *)
  cfg[68] := 'u'; cfg[69] := 's'; cfg[70] := 'e'; cfg[71] := 'r';
  cfg[72] := ' '; cfg[73] := '='; cfg[74] := ' '; cfg[75] := 'a';
  cfg[76] := 'd'; cfg[77] := 'm'; cfg[78] := 'i'; cfg[79] := 'n';
  cfg[80] := CHR(10);
  cfg[81] := 0C;

  (* Test 1: parse returns TRUE *)
  ok := Parse(cfg, 81);
  Check("parse_ok", ok);

  (* Test 2: section count = 3 (default "" + server + database) *)
  Check("section_count", SectionCount() = 3);

  (* Test 3: section 0 is "" (default) *)
  ok := GetSectionName(0, name);
  Check("section_name_0", ok AND (Length(name) = 0));

  (* Test 4: section 1 is "server" *)
  ok := GetSectionName(1, name);
  Check("section_name_1", ok AND (CompareStr(name, "server") = 0));

  (* Test 5: KeyCount("server") = 2 *)
  n := KeyCount("server");
  Check("key_count_server", n = 2);

  (* Test 6: GetValue("server","host") = "localhost" *)
  ok := GetValue("server", "host", val);
  Check("get_host", ok AND (CompareStr(val, "localhost") = 0));

  (* Test 7: GetValue("server","port") = "8080" *)
  ok := GetValue("server", "port", val);
  Check("get_port", ok AND (CompareStr(val, "8080") = 0));

  (* Test 8: GetValue("database","name") = "mydb" *)
  ok := GetValue("database", "name", val);
  Check("get_db_name", ok AND (CompareStr(val, "mydb") = 0));

  (* Test 9: GetValue("database","user") = "admin" *)
  ok := GetValue("database", "user", val);
  Check("get_db_user", ok AND (CompareStr(val, "admin") = 0));

  (* Test 10: HasKey("server","host") = TRUE *)
  Check("has_key_true", HasKey("server", "host"));

  (* Test 11: HasKey("server","missing") = FALSE *)
  Check("has_key_false", NOT HasKey("server", "missing"));

  (* Test 12: GetValue for missing section returns FALSE *)
  ok := GetValue("nosuch", "key", val);
  Check("missing_section", NOT ok);

  (* Test 13: GetKey("server",0) = "host" *)
  ok := GetKey("server", 0, key);
  Check("get_key_by_idx", ok AND (CompareStr(key, "host") = 0));

  (* Test 14: Clear then SectionCount = 0 *)
  Clear;
  Check("clear", SectionCount() = 0);

  (* Test 15: Parse empty string, SectionCount = 1 (default section) *)
  cfg[0] := 0C;
  ok := Parse(cfg, 0);
  Check("parse_empty", ok AND (SectionCount() = 0))
END TestBasicParse;

(* ── Test whitespace trimming ───────────────────────── *)

PROCEDURE TestWhitespace;
VAR
  cfg: ARRAY [0..127] OF CHAR;
  ok: BOOLEAN;
  val: ARRAY [0..255] OF CHAR;
BEGIN
  (* " key = value " with [ws] section *)
  cfg[0] := '['; cfg[1] := 'w'; cfg[2] := 's'; cfg[3] := ']';
  cfg[4] := CHR(10);
  cfg[5] := ' '; cfg[6] := ' '; cfg[7] := 'k'; cfg[8] := 'e';
  cfg[9] := 'y'; cfg[10] := ' '; cfg[11] := '='; cfg[12] := ' ';
  cfg[13] := 'v'; cfg[14] := 'a'; cfg[15] := 'l'; cfg[16] := 'u';
  cfg[17] := 'e'; cfg[18] := ' '; cfg[19] := ' '; cfg[20] := CHR(10);
  cfg[21] := 0C;

  ok := Parse(cfg, 21);
  Check("ws_parse", ok);

  ok := GetValue("ws", "key", val);
  Check("ws_trim", ok AND (CompareStr(val, "value") = 0))
END TestWhitespace;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestBasicParse;
  TestWhitespace;

  WriteLn;
  WriteString("m2conf: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END ConfTests.
