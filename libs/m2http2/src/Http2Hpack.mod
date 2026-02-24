IMPLEMENTATION MODULE Http2Hpack;

FROM Http2Types IMPORT HeaderEntry, HeaderName, HeaderValue,
                       HpackStaticTableSize, HpackMaxDynEntries,
                       HpackMaxNameLen, HpackMaxValueLen;
IMPORT ByteBuf;
FROM ByteBuf IMPORT BytesView, Buf;

(* ── Static table (RFC 7541 Appendix A) ────────────────── *)

VAR
  staticTable: ARRAY [0..61] OF HeaderEntry;
  staticReady: BOOLEAN;

PROCEDURE SetEntry(VAR e: HeaderEntry;
                   n: ARRAY OF CHAR; nl: CARDINAL;
                   v: ARRAY OF CHAR; vl: CARDINAL);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < nl DO
    e.name[i] := n[i];
    INC(i)
  END;
  e.nameLen := nl;
  i := 0;
  WHILE i < vl DO
    e.value[i] := v[i];
    INC(i)
  END;
  e.valLen := vl
END SetEntry;

PROCEDURE EntryNameEq(VAR e: HeaderEntry;
                      n: ARRAY OF CHAR; nl: CARDINAL): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  IF e.nameLen # nl THEN RETURN FALSE END;
  i := 0;
  WHILE i < nl DO
    IF e.name[i] # n[i] THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END EntryNameEq;

PROCEDURE EntryValEq(VAR e: HeaderEntry;
                     v: ARRAY OF CHAR; vl: CARDINAL): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  IF e.valLen # vl THEN RETURN FALSE END;
  i := 0;
  WHILE i < vl DO
    IF e.value[i] # v[i] THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END EntryValEq;

PROCEDURE InitStaticTable;
BEGIN
  IF staticReady THEN RETURN END;
  (* Index 0 is unused; indices 1..61 per RFC 7541 Appendix A. *)
  SetEntry(staticTable[0], "", 0, "", 0);
  SetEntry(staticTable[1], ":authority", 10, "", 0);
  SetEntry(staticTable[2], ":method", 7, "GET", 3);
  SetEntry(staticTable[3], ":method", 7, "POST", 4);
  SetEntry(staticTable[4], ":path", 5, "/", 1);
  SetEntry(staticTable[5], ":path", 5, "/index.html", 11);
  SetEntry(staticTable[6], ":scheme", 7, "http", 4);
  SetEntry(staticTable[7], ":scheme", 7, "https", 5);
  SetEntry(staticTable[8], ":status", 7, "200", 3);
  SetEntry(staticTable[9], ":status", 7, "204", 3);
  SetEntry(staticTable[10], ":status", 7, "206", 3);
  SetEntry(staticTable[11], ":status", 7, "304", 3);
  SetEntry(staticTable[12], ":status", 7, "400", 3);
  SetEntry(staticTable[13], ":status", 7, "404", 3);
  SetEntry(staticTable[14], ":status", 7, "500", 3);
  SetEntry(staticTable[15], "accept-charset", 14, "", 0);
  SetEntry(staticTable[16], "accept-encoding", 15, "gzip, deflate", 13);
  SetEntry(staticTable[17], "accept-language", 15, "", 0);
  SetEntry(staticTable[18], "accept-ranges", 13, "", 0);
  SetEntry(staticTable[19], "accept", 6, "", 0);
  SetEntry(staticTable[20], "access-control-allow-origin", 27, "", 0);
  SetEntry(staticTable[21], "age", 3, "", 0);
  SetEntry(staticTable[22], "allow", 5, "", 0);
  SetEntry(staticTable[23], "authorization", 13, "", 0);
  SetEntry(staticTable[24], "cache-control", 13, "", 0);
  SetEntry(staticTable[25], "content-disposition", 19, "", 0);
  SetEntry(staticTable[26], "content-encoding", 16, "", 0);
  SetEntry(staticTable[27], "content-language", 16, "", 0);
  SetEntry(staticTable[28], "content-length", 14, "", 0);
  SetEntry(staticTable[29], "content-location", 16, "", 0);
  SetEntry(staticTable[30], "content-range", 13, "", 0);
  SetEntry(staticTable[31], "content-type", 12, "", 0);
  SetEntry(staticTable[32], "cookie", 6, "", 0);
  SetEntry(staticTable[33], "date", 4, "", 0);
  SetEntry(staticTable[34], "etag", 4, "", 0);
  SetEntry(staticTable[35], "expect", 6, "", 0);
  SetEntry(staticTable[36], "expires", 7, "", 0);
  SetEntry(staticTable[37], "from", 4, "", 0);
  SetEntry(staticTable[38], "host", 4, "", 0);
  SetEntry(staticTable[39], "if-match", 8, "", 0);
  SetEntry(staticTable[40], "if-modified-since", 17, "", 0);
  SetEntry(staticTable[41], "if-none-match", 13, "", 0);
  SetEntry(staticTable[42], "if-range", 8, "", 0);
  SetEntry(staticTable[43], "if-unmodified-since", 19, "", 0);
  SetEntry(staticTable[44], "last-modified", 13, "", 0);
  SetEntry(staticTable[45], "link", 4, "", 0);
  SetEntry(staticTable[46], "location", 8, "", 0);
  SetEntry(staticTable[47], "max-forwards", 12, "", 0);
  SetEntry(staticTable[48], "proxy-authenticate", 18, "", 0);
  SetEntry(staticTable[49], "proxy-authorization", 19, "", 0);
  SetEntry(staticTable[50], "range", 5, "", 0);
  SetEntry(staticTable[51], "referer", 7, "", 0);
  SetEntry(staticTable[52], "refresh", 7, "", 0);
  SetEntry(staticTable[53], "retry-after", 11, "", 0);
  SetEntry(staticTable[54], "server", 6, "", 0);
  SetEntry(staticTable[55], "set-cookie", 10, "", 0);
  SetEntry(staticTable[56], "strict-transport-security", 25, "", 0);
  SetEntry(staticTable[57], "transfer-encoding", 17, "", 0);
  SetEntry(staticTable[58], "user-agent", 10, "", 0);
  SetEntry(staticTable[59], "vary", 4, "", 0);
  SetEntry(staticTable[60], "via", 3, "", 0);
  SetEntry(staticTable[61], "www-authenticate", 16, "", 0);
  staticReady := TRUE
END InitStaticTable;

(* ── Integer codec (RFC 7541 Section 5.1) ──────────────── *)

PROCEDURE PrefixMax(prefixBits: CARDINAL): CARDINAL;
BEGIN
  CASE prefixBits OF
    1: RETURN 1   | 2: RETURN 3   | 3: RETURN 7
  | 4: RETURN 15  | 5: RETURN 31  | 6: RETURN 63
  | 7: RETURN 127 | 8: RETURN 255
  ELSE RETURN 0
  END
END PrefixMax;

PROCEDURE EncodeInt(VAR b: Buf; value: CARDINAL;
                    prefixBits: CARDINAL; mask: CARDINAL);
VAR maxP, v: CARDINAL;
BEGIN
  maxP := PrefixMax(prefixBits);
  IF value < maxP THEN
    ByteBuf.AppendByte(b, mask + value)
  ELSE
    ByteBuf.AppendByte(b, mask + maxP);
    v := value - maxP;
    WHILE v >= 128 DO
      ByteBuf.AppendByte(b, (v MOD 128) + 128);
      v := v DIV 128
    END;
    ByteBuf.AppendByte(b, v)
  END
END EncodeInt;

PROCEDURE DecodeInt(firstByte: CARDINAL; prefixBits: CARDINAL;
                    v: BytesView; VAR pos: CARDINAL;
                    VAR ok: BOOLEAN): CARDINAL;
VAR maxP, result, byt, mult: CARDINAL;
BEGIN
  ok := TRUE;
  maxP := PrefixMax(prefixBits);
  result := firstByte;
  IF result < maxP THEN
    RETURN result
  END;
  (* Multi-byte: result starts at maxP, accumulate continuation *)
  result := maxP;
  mult := 1;  (* multiplier: 1, 128, 16384, ... *)
  LOOP
    IF pos >= v.len THEN
      ok := FALSE;
      RETURN 0
    END;
    byt := ByteBuf.ViewGetByte(v, pos);
    INC(pos);
    result := result + (byt MOD 128) * mult;
    IF byt < 128 THEN
      EXIT
    END;
    mult := mult * 128;
    IF mult > 268435456 THEN  (* 128^4, overflow guard *)
      ok := FALSE;
      RETURN 0
    END
  END;
  RETURN result
END DecodeInt;

(* ── Static table ──────────────────────────────────────── *)

PROCEDURE StaticLookup(index: CARDINAL;
                       VAR entry: HeaderEntry;
                       VAR ok: BOOLEAN);
VAR i: CARDINAL;
BEGIN
  InitStaticTable;
  ok := (index >= 1) AND (index <= HpackStaticTableSize);
  IF NOT ok THEN RETURN END;
  i := 0;
  WHILE i < staticTable[index].nameLen DO
    entry.name[i] := staticTable[index].name[i];
    INC(i)
  END;
  entry.nameLen := staticTable[index].nameLen;
  i := 0;
  WHILE i < staticTable[index].valLen DO
    entry.value[i] := staticTable[index].value[i];
    INC(i)
  END;
  entry.valLen := staticTable[index].valLen
END StaticLookup;

PROCEDURE StaticFind(name: ARRAY OF CHAR; nameLen: CARDINAL;
                     value: ARRAY OF CHAR; valLen: CARDINAL;
                     nameOnly: BOOLEAN): CARDINAL;
VAR i, lo, hi, nameMatch: CARDINAL;
    ch: CHAR;
BEGIN
  InitStaticTable;
  IF nameLen = 0 THEN RETURN 0 END;
  ch := name[0];
  (* First-character dispatch: narrow scan to entries starting with ch *)
  CASE ch OF
    ':': lo := 1;  hi := 14
  | 'a': lo := 15; hi := 23
  | 'c': lo := 24; hi := 32
  | 'd': lo := 33; hi := 33
  | 'e': lo := 34; hi := 36
  | 'f': lo := 37; hi := 37
  | 'h': lo := 38; hi := 38
  | 'i': lo := 39; hi := 43
  | 'l': lo := 44; hi := 46
  | 'm': lo := 47; hi := 47
  | 'p': lo := 48; hi := 49
  | 'r': lo := 50; hi := 53
  | 's': lo := 54; hi := 56
  | 't': lo := 57; hi := 57
  | 'u': lo := 58; hi := 58
  | 'v': lo := 59; hi := 60
  | 'w': lo := 61; hi := 61
  ELSE RETURN 0
  END;
  nameMatch := 0;
  i := lo;
  WHILE i <= hi DO
    IF EntryNameEq(staticTable[i], name, nameLen) THEN
      IF nameOnly THEN
        RETURN i
      END;
      IF nameMatch = 0 THEN nameMatch := i END;
      IF EntryValEq(staticTable[i], value, valLen) THEN
        RETURN i  (* Exact match *)
      END
    END;
    INC(i)
  END;
  RETURN nameMatch
END StaticFind;

(* ── Dynamic table ─────────────────────────────────────── *)

PROCEDURE EntrySize(nameLen, valLen: CARDINAL): CARDINAL;
BEGIN
  RETURN nameLen + valLen + 32
END EntrySize;

PROCEDURE DynInit(VAR dt: DynTable; maxSize: CARDINAL);
BEGIN
  dt.head := 0;
  dt.count := 0;
  dt.byteSize := 0;
  dt.maxSize := maxSize
END DynInit;

PROCEDURE DynRealIndex(VAR dt: DynTable; index: CARDINAL): CARDINAL;
(* Convert 0-based index (newest=0) to ring buffer slot. *)
BEGIN
  IF index <= dt.head THEN
    RETURN dt.head - index
  ELSE
    RETURN HpackMaxDynEntries - (index - dt.head)
  END
END DynRealIndex;

PROCEDURE DynEvict(VAR dt: DynTable);
VAR oldest, eSize: CARDINAL;
BEGIN
  IF dt.count = 0 THEN RETURN END;
  oldest := DynRealIndex(dt, dt.count - 1);
  eSize := EntrySize(dt.entries[oldest].nameLen,
                     dt.entries[oldest].valLen);
  IF dt.byteSize >= eSize THEN
    dt.byteSize := dt.byteSize - eSize
  ELSE
    dt.byteSize := 0
  END;
  DEC(dt.count)
END DynEvict;

PROCEDURE DynInsert(VAR dt: DynTable;
                    name: ARRAY OF CHAR; nameLen: CARDINAL;
                    value: ARRAY OF CHAR; valLen: CARDINAL);
VAR eSize, slot, i: CARDINAL;
BEGIN
  eSize := EntrySize(nameLen, valLen);
  IF eSize > dt.maxSize THEN
    dt.count := 0;
    dt.byteSize := 0;
    RETURN
  END;
  WHILE (dt.count > 0) AND (dt.byteSize + eSize > dt.maxSize) DO
    DynEvict(dt)
  END;
  IF dt.count > 0 THEN
    IF dt.head = HpackMaxDynEntries - 1 THEN
      dt.head := 0
    ELSE
      INC(dt.head)
    END
  END;
  slot := dt.head;
  i := 0;
  WHILE i < nameLen DO
    dt.entries[slot].name[i] := name[i];
    INC(i)
  END;
  dt.entries[slot].nameLen := nameLen;
  i := 0;
  WHILE i < valLen DO
    dt.entries[slot].value[i] := value[i];
    INC(i)
  END;
  dt.entries[slot].valLen := valLen;
  INC(dt.count);
  dt.byteSize := dt.byteSize + eSize
END DynInsert;

PROCEDURE DynLookup(VAR dt: DynTable; index: CARDINAL;
                    VAR entry: HeaderEntry; VAR ok: BOOLEAN);
VAR slot, i: CARDINAL;
BEGIN
  ok := index < dt.count;
  IF NOT ok THEN RETURN END;
  slot := DynRealIndex(dt, index);
  i := 0;
  WHILE i < dt.entries[slot].nameLen DO
    entry.name[i] := dt.entries[slot].name[i];
    INC(i)
  END;
  entry.nameLen := dt.entries[slot].nameLen;
  i := 0;
  WHILE i < dt.entries[slot].valLen DO
    entry.value[i] := dt.entries[slot].value[i];
    INC(i)
  END;
  entry.valLen := dt.entries[slot].valLen
END DynLookup;

PROCEDURE DynResize(VAR dt: DynTable; newMaxSize: CARDINAL);
BEGIN
  dt.maxSize := newMaxSize;
  WHILE (dt.count > 0) AND (dt.byteSize > dt.maxSize) DO
    DynEvict(dt)
  END
END DynResize;

PROCEDURE DynCount(VAR dt: DynTable): CARDINAL;
BEGIN
  RETURN dt.count
END DynCount;

(* ── Header block decode ───────────────────────────────── *)

PROCEDURE CopyViewToArr(v: BytesView; start, n: CARDINAL;
                        VAR a: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < n DO
    a[i] := CHR(ByteBuf.ViewGetByte(v, start + i));
    INC(i)
  END
END CopyViewToArr;

PROCEDURE CopyEntryName(VAR src, dst: HeaderEntry);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < src.nameLen DO
    dst.name[i] := src.name[i];
    INC(i)
  END;
  dst.nameLen := src.nameLen
END CopyEntryName;

PROCEDURE LookupIndex(idx: CARDINAL; VAR dt: DynTable;
                      VAR entry: HeaderEntry; VAR ok: BOOLEAN);
BEGIN
  IF idx <= HpackStaticTableSize THEN
    StaticLookup(idx, entry, ok)
  ELSE
    DynLookup(dt, idx - HpackStaticTableSize - 1, entry, ok)
  END
END LookupIndex;

PROCEDURE StaticLookupName(index: CARDINAL;
                           VAR entry: HeaderEntry;
                           VAR ok: BOOLEAN);
VAR i: CARDINAL;
BEGIN
  InitStaticTable;
  ok := (index >= 1) AND (index <= HpackStaticTableSize);
  IF NOT ok THEN RETURN END;
  i := 0;
  WHILE i < staticTable[index].nameLen DO
    entry.name[i] := staticTable[index].name[i];
    INC(i)
  END;
  entry.nameLen := staticTable[index].nameLen
END StaticLookupName;

PROCEDURE DynLookupName(VAR dt: DynTable; index: CARDINAL;
                        VAR entry: HeaderEntry; VAR ok: BOOLEAN);
VAR slot, i: CARDINAL;
BEGIN
  ok := index < dt.count;
  IF NOT ok THEN RETURN END;
  slot := DynRealIndex(dt, index);
  i := 0;
  WHILE i < dt.entries[slot].nameLen DO
    entry.name[i] := dt.entries[slot].name[i];
    INC(i)
  END;
  entry.nameLen := dt.entries[slot].nameLen
END DynLookupName;

PROCEDURE LookupNameOnly(idx: CARDINAL; VAR dt: DynTable;
                         VAR entry: HeaderEntry; VAR ok: BOOLEAN);
BEGIN
  IF idx <= HpackStaticTableSize THEN
    StaticLookupName(idx, entry, ok)
  ELSE
    DynLookupName(dt, idx - HpackStaticTableSize - 1, entry, ok)
  END
END LookupNameOnly;

(* ── Huffman decode tree (RFC 7541 Appendix B) ──────────── *)

CONST
  HuffMaxNodes = 600;  (* 257 leaves + 256 internal + margin *)
  HuffEOS = 256;

TYPE
  HuffAccelEntry = RECORD
    sym:        INTEGER;    (* first decoded symbol, -1 if code > 8 bits *)
    bitsUsed:   CARDINAL;   (* bits consumed for that symbol *)
    nodeAfter8: CARDINAL;   (* tree node after all 8 bits, when sym=-1 *)
  END;

VAR
  huffLeft:  ARRAY [0..HuffMaxNodes-1] OF INTEGER;
  huffRight: ARRAY [0..HuffMaxNodes-1] OF INTEGER;
  huffSym:   ARRAY [0..HuffMaxNodes-1] OF INTEGER;
  huffNodes: CARDINAL;
  huffReady: BOOLEAN;
  huffAccel: ARRAY [0..255] OF HuffAccelEntry;

PROCEDURE HuffInsert(code: LONGCARD; bits, sym: CARDINAL);
(* Insert symbol into the Huffman decode tree.
   code is left-aligned: MSB (bit 31) is the first bit to decode. *)
VAR
  node: CARDINAL;
  i: CARDINAL;
  mask: LONGCARD;
  child: INTEGER;
BEGIN
  node := 0;
  mask := 2147483648;  (* 2^31 *)
  FOR i := 0 TO bits - 1 DO
    IF (code DIV mask) MOD 2 = 1 THEN
      (* go right *)
      child := huffRight[node];
      IF child = -1 THEN
        child := INTEGER(huffNodes);
        huffLeft[huffNodes] := -1;
        huffRight[huffNodes] := -1;
        huffSym[huffNodes] := -1;
        huffRight[node] := child;
        INC(huffNodes)
      END
    ELSE
      (* go left *)
      child := huffLeft[node];
      IF child = -1 THEN
        child := INTEGER(huffNodes);
        huffLeft[huffNodes] := -1;
        huffRight[huffNodes] := -1;
        huffSym[huffNodes] := -1;
        huffLeft[node] := child;
        INC(huffNodes)
      END
    END;
    node := CARDINAL(child);
    mask := mask DIV 2
  END;
  huffSym[node] := INTEGER(sym)
END HuffInsert;

PROCEDURE I(code: LONGCARD; bits, sym: CARDINAL);
BEGIN HuffInsert(code, bits, sym) END I;

PROCEDURE BuildHuffAccel;
(* Build 256-entry root acceleration table. For each byte value 0..255,
   walk tree from root consuming bits. If a leaf is reached within 8 bits,
   record sym and bitsUsed. Otherwise record nodeAfter8. *)
VAR
  byteVal, bitsConsumed, node: CARDINAL;
  b: CARDINAL;
BEGIN
  FOR byteVal := 0 TO 255 DO
    node := 0;
    b := byteVal;
    bitsConsumed := 0;
    huffAccel[byteVal].sym := -1;
    huffAccel[byteVal].bitsUsed := 0;
    huffAccel[byteVal].nodeAfter8 := 0;
    WHILE bitsConsumed < 8 DO
      IF b >= 128 THEN
        node := CARDINAL(huffRight[node])
      ELSE
        node := CARDINAL(huffLeft[node])
      END;
      b := (b MOD 128) * 2;
      INC(bitsConsumed);
      IF huffSym[node] >= 0 THEN
        huffAccel[byteVal].sym := huffSym[node];
        huffAccel[byteVal].bitsUsed := bitsConsumed;
        bitsConsumed := 8  (* break *)
      END
    END;
    IF huffAccel[byteVal].sym = -1 THEN
      huffAccel[byteVal].nodeAfter8 := node
    END
  END
END BuildHuffAccel;

PROCEDURE InitHuffTree;
BEGIN
  IF huffReady THEN RETURN END;
  huffLeft[0] := -1;
  huffRight[0] := -1;
  huffSym[0] := -1;
  huffNodes := 1;

  (* Control codes 0-31 *)
  I(4290772992,13,0); I(4294946816,23,1);
  I(4294966816,28,2); I(4294966832,28,3);
  I(4294966848,28,4); I(4294966864,28,5);
  I(4294966880,28,6); I(4294966896,28,7);
  I(4294966912,28,8); I(4294961664,24,9);
  I(4294967280,30,10); I(4294966928,28,11);
  I(4294966944,28,12); I(4294967284,30,13);
  I(4294966960,28,14); I(4294966976,28,15);
  I(4294966992,28,16); I(4294967008,28,17);
  I(4294967024,28,18); I(4294967040,28,19);
  I(4294967056,28,20); I(4294967072,28,21);
  I(4294967288,30,22); I(4294967088,28,23);
  I(4294967104,28,24); I(4294967120,28,25);
  I(4294967136,28,26); I(4294967152,28,27);
  I(4294967168,28,28); I(4294967184,28,29);
  I(4294967200,28,30); I(4294967216,28,31);

  (* Printable ASCII 32-126 *)
  I(1342177280,6,32); I(4261412864,10,33);
  I(4265607168,10,34); I(4288675840,12,35);
  I(4291297280,13,36); I(1409286144,6,37);
  I(4160749568,8,38); I(4282384384,11,39);
  I(4269801472,10,40); I(4273995776,10,41);
  I(4177526784,8,42); I(4284481536,11,43);
  I(4194304000,8,44); I(1476395008,6,45);
  I(1543503872,6,46); I(1610612736,6,47);
  I(0,5,48); I(134217728,5,49);
  I(268435456,5,50); I(1677721600,6,51);
  I(1744830464,6,52); I(1811939328,6,53);
  I(1879048192,6,54); I(1946157056,6,55);
  I(2013265920,6,56); I(2080374784,6,57);
  I(3087007744,7,58); I(4211081216,8,59);
  I(4294443008,15,60); I(2147483648,6,61);
  I(4289724416,12,62); I(4278190080,10,63);
  I(4291821568,13,64); I(2214592512,6,65);
  I(3120562176,7,66); I(3154116608,7,67);
  I(3187671040,7,68); I(3221225472,7,69);
  I(3254779904,7,70); I(3288334336,7,71);
  I(3321888768,7,72); I(3355443200,7,73);
  I(3388997632,7,74); I(3422552064,7,75);
  I(3456106496,7,76); I(3489660928,7,77);
  I(3523215360,7,78); I(3556769792,7,79);
  I(3590324224,7,80); I(3623878656,7,81);
  I(3657433088,7,82); I(3690987520,7,83);
  I(3724541952,7,84); I(3758096384,7,85);
  I(3791650816,7,86); I(3825205248,7,87);
  I(4227858432,8,88); I(3858759680,7,89);
  I(4244635648,8,90); I(4292345856,13,91);
  I(4294836224,19,92); I(4292870144,13,93);
  I(4293918720,14,94); I(2281701376,6,95);
  I(4294574080,15,96); I(402653184,5,97);
  I(2348810240,6,98); I(536870912,5,99);
  I(2415919104,6,100); I(671088640,5,101);
  I(2483027968,6,102); I(2550136832,6,103);
  I(2617245696,6,104); I(805306368,5,105);
  I(3892314112,7,106); I(3925868544,7,107);
  I(2684354560,6,108); I(2751463424,6,109);
  I(2818572288,6,110); I(939524096,5,111);
  I(2885681152,6,112); I(3959422976,7,113);
  I(2952790016,6,114); I(1073741824,5,115);
  I(1207959552,5,116); I(3019898880,6,117);
  I(3992977408,7,118); I(4026531840,7,119);
  I(4060086272,7,120); I(4093640704,7,121);
  I(4127195136,7,122); I(4294705152,15,123);
  I(4286578688,11,124); I(4294180864,14,125);
  I(4293394432,13,126);

  (* High bytes 127-255 *)
  I(4294967232,28,127); I(4294860800,20,128);
  I(4294920192,22,129); I(4294864896,20,130);
  I(4294868992,20,131); I(4294921216,22,132);
  I(4294922240,22,133); I(4294923264,22,134);
  I(4294947328,23,135); I(4294924288,22,136);
  I(4294947840,23,137); I(4294948352,23,138);
  I(4294948864,23,139); I(4294949376,23,140);
  I(4294949888,23,141); I(4294961920,24,142);
  I(4294950400,23,143); I(4294962176,24,144);
  I(4294962432,24,145); I(4294925312,22,146);
  I(4294950912,23,147); I(4294962688,24,148);
  I(4294951424,23,149); I(4294951936,23,150);
  I(4294952448,23,151); I(4294952960,23,152);
  I(4294893568,21,153); I(4294926336,22,154);
  I(4294953472,23,155); I(4294927360,22,156);
  I(4294953984,23,157); I(4294954496,23,158);
  I(4294962944,24,159); I(4294928384,22,160);
  I(4294895616,21,161); I(4294873088,20,162);
  I(4294929408,22,163); I(4294930432,22,164);
  I(4294955008,23,165); I(4294955520,23,166);
  I(4294897664,21,167); I(4294956032,23,168);
  I(4294931456,22,169); I(4294932480,22,170);
  I(4294963200,24,171); I(4294899712,21,172);
  I(4294933504,22,173); I(4294956544,23,174);
  I(4294957056,23,175); I(4294901760,21,176);
  I(4294903808,21,177); I(4294934528,22,178);
  I(4294905856,21,179); I(4294957568,23,180);
  I(4294935552,22,181); I(4294958080,23,182);
  I(4294958592,23,183); I(4294877184,20,184);
  I(4294936576,22,185); I(4294937600,22,186);
  I(4294938624,22,187); I(4294959104,23,188);
  I(4294939648,22,189); I(4294940672,22,190);
  I(4294959616,23,191); I(4294965248,26,192);
  I(4294965312,26,193); I(4294881280,20,194);
  I(4294844416,19,195); I(4294941696,22,196);
  I(4294960128,23,197); I(4294942720,22,198);
  I(4294964736,25,199); I(4294965376,26,200);
  I(4294965440,26,201); I(4294965504,26,202);
  I(4294966208,27,203); I(4294966240,27,204);
  I(4294965568,26,205); I(4294963456,24,206);
  I(4294964864,25,207); I(4294852608,19,208);
  I(4294907904,21,209); I(4294965632,26,210);
  I(4294966272,27,211); I(4294966304,27,212);
  I(4294965696,26,213); I(4294966336,27,214);
  I(4294963712,24,215); I(4294909952,21,216);
  I(4294912000,21,217); I(4294965760,26,218);
  I(4294965824,26,219); I(4294967248,28,220);
  I(4294966368,27,221); I(4294966400,27,222);
  I(4294966432,27,223); I(4294885376,20,224);
  I(4294963968,24,225); I(4294889472,20,226);
  I(4294914048,21,227); I(4294943744,22,228);
  I(4294916096,21,229); I(4294918144,21,230);
  I(4294960640,23,231); I(4294944768,22,232);
  I(4294945792,22,233); I(4294964992,25,234);
  I(4294965120,25,235); I(4294964224,24,236);
  I(4294964480,24,237); I(4294965888,26,238);
  I(4294961152,23,239); I(4294965952,26,240);
  I(4294966464,27,241); I(4294966016,26,242);
  I(4294966080,26,243); I(4294966496,27,244);
  I(4294966528,27,245); I(4294966560,27,246);
  I(4294966592,27,247); I(4294966624,27,248);
  I(4294967264,28,249); I(4294966656,27,250);
  I(4294966688,27,251); I(4294966720,27,252);
  I(4294966752,27,253); I(4294966784,27,254);
  I(4294966144,26,255);

  (* EOS *)
  I(4294967292,30,256);

  BuildHuffAccel;
  huffReady := TRUE
END InitHuffTree;

(* Walk one bit through the Huffman tree.
   Returns FALSE if the tree has no child for this bit. *)
PROCEDURE HuffWalkBit(bit: CARDINAL;
                      VAR node: CARDINAL;
                      VAR arr: ARRAY OF CHAR;
                      VAR decLen: CARDINAL;
                      maxLen: CARDINAL;
                      VAR ok: BOOLEAN);
VAR sym: INTEGER;
BEGIN
  IF bit = 1 THEN
    IF huffRight[node] = -1 THEN ok := FALSE; RETURN END;
    node := CARDINAL(huffRight[node])
  ELSE
    IF huffLeft[node] = -1 THEN ok := FALSE; RETURN END;
    node := CARDINAL(huffLeft[node])
  END;
  sym := huffSym[node];
  IF sym >= 0 THEN
    IF CARDINAL(sym) = HuffEOS THEN ok := FALSE; RETURN END;
    IF decLen >= maxLen THEN ok := FALSE; RETURN END;
    arr[decLen] := CHR(CARDINAL(sym) MOD 256);
    INC(decLen);
    node := 0
  END
END HuffWalkBit;

(* Decode Huffman-encoded bytes from v[start..start+encLen-1]
   into arr, writing at most maxLen bytes.
   Sets decLen to the number of decoded bytes and ok to TRUE on success. *)
PROCEDURE HuffDecode(v: BytesView; start, encLen: CARDINAL;
                     VAR arr: ARRAY OF CHAR;
                     VAR decLen: CARDINAL;
                     maxLen: CARDINAL;
                     VAR ok: BOOLEAN);
VAR
  node: CARDINAL;
  bIdx, endIdx: CARDINAL;
  byt: CARDINAL;
  rem: CARDINAL;
  i: CARDINAL;
  a: HuffAccelEntry;
BEGIN
  InitHuffTree;
  ok := TRUE;
  decLen := 0;
  node := 0;
  bIdx := start;
  endIdx := start + encLen;

  WHILE (bIdx < endIdx) AND ok DO
    byt := ByteBuf.ViewGetByte(v, bIdx);
    INC(bIdx);

    IF node = 0 THEN
      (* At root: use 8-bit acceleration table *)
      a := huffAccel[byt];
      IF a.sym >= 0 THEN
        (* Short code (<=8 bits): emit symbol *)
        IF CARDINAL(a.sym) = HuffEOS THEN ok := FALSE; RETURN END;
        IF decLen >= maxLen THEN ok := FALSE; RETURN END;
        arr[decLen] := CHR(CARDINAL(a.sym) MOD 256);
        INC(decLen);
        node := 0;
        (* Shift out consumed bits and process remainder via tree walk *)
        CASE a.bitsUsed OF
          1: byt := (byt MOD 128) * 2;   rem := 7
        | 2: byt := (byt MOD 64) * 4;    rem := 6
        | 3: byt := (byt MOD 32) * 8;    rem := 5
        | 4: byt := (byt MOD 16) * 16;   rem := 4
        | 5: byt := (byt MOD 8) * 32;    rem := 3
        | 6: byt := (byt MOD 4) * 64;    rem := 2
        | 7: byt := (byt MOD 2) * 128;   rem := 1
        | 8: rem := 0
        ELSE rem := 0
        END;
        i := 0;
        WHILE (i < rem) AND ok DO
          HuffWalkBit(byt DIV 128, node, arr, decLen, maxLen, ok);
          byt := (byt MOD 128) * 2;
          INC(i)
        END
      ELSE
        (* Long code (>8 bits): advance to tree node after 8 bits *)
        node := a.nodeAfter8
      END
    ELSE
      (* Not at root: process 8 bits individually *)
      i := 0;
      WHILE (i < 8) AND ok DO
        HuffWalkBit(byt DIV 128, node, arr, decLen, maxLen, ok);
        byt := (byt MOD 128) * 2;
        INC(i)
      END
    END
  END;

  (* Padding: accept if remaining bits are all-1s padding *)
  IF ok AND (node # 0) THEN
    (* Proper encoders only add all-1 padding which won't complete
       a symbol within 7 bits, so accept. *)
  END
END HuffDecode;

PROCEDURE ReadLiteralString(v: BytesView; VAR pos: CARDINAL;
                            VAR arr: ARRAY OF CHAR;
                            VAR sLen: CARDINAL;
                            maxLen: CARDINAL;
                            VAR ok: BOOLEAN);
VAR byt, encLen: CARDINAL; isHuffman: BOOLEAN;
BEGIN
  IF pos >= v.len THEN ok := FALSE; RETURN END;
  byt := ByteBuf.ViewGetByte(v, pos);
  INC(pos);
  isHuffman := byt >= 128;
  encLen := DecodeInt(byt MOD 128, 7, v, pos, ok);
  IF NOT ok THEN RETURN END;
  IF pos + encLen > v.len THEN ok := FALSE; RETURN END;
  IF isHuffman THEN
    HuffDecode(v, pos, encLen, arr, sLen, maxLen, ok);
    INC(pos, encLen)
  ELSE
    IF encLen > maxLen THEN ok := FALSE; RETURN END;
    CopyViewToArr(v, pos, encLen, arr);
    sLen := encLen;
    INC(pos, encLen)
  END
END ReadLiteralString;

PROCEDURE DecodeHeaderBlock(v: BytesView;
                            VAR dt: DynTable;
                            VAR headers: ARRAY OF HeaderEntry;
                            maxOut: CARDINAL;
                            VAR numHeaders: CARDINAL;
                            VAR ok: BOOLEAN);
VAR pos, byt, idx: CARDINAL;
BEGIN
  InitStaticTable;
  ok := TRUE;
  numHeaders := 0;
  pos := 0;

  WHILE (pos < v.len) AND ok AND (numHeaders < maxOut) DO
    byt := ByteBuf.ViewGetByte(v, pos);
    INC(pos);

    IF byt >= 128 THEN
      (* 6.1: Indexed Header Field — 1-bit prefix at bit 7 *)
      idx := DecodeInt(byt MOD 128, 7, v, pos, ok);
      IF NOT ok THEN RETURN END;
      IF idx = 0 THEN ok := FALSE; RETURN END;
      LookupIndex(idx, dt, headers[numHeaders], ok);
      IF NOT ok THEN RETURN END;
      INC(numHeaders)

    ELSIF byt >= 64 THEN
      (* 6.2.1: Literal with Incremental Indexing — 6-bit prefix *)
      idx := DecodeInt(byt MOD 64, 6, v, pos, ok);
      IF NOT ok THEN RETURN END;
      IF idx > 0 THEN
        LookupNameOnly(idx, dt, headers[numHeaders], ok);
        IF NOT ok THEN RETURN END
      ELSE
        ReadLiteralString(v, pos, headers[numHeaders].name,
                          headers[numHeaders].nameLen,
                          HpackMaxNameLen, ok);
        IF NOT ok THEN RETURN END
      END;
      ReadLiteralString(v, pos, headers[numHeaders].value,
                        headers[numHeaders].valLen,
                        HpackMaxValueLen, ok);
      IF NOT ok THEN RETURN END;
      DynInsert(dt, headers[numHeaders].name,
                headers[numHeaders].nameLen,
                headers[numHeaders].value,
                headers[numHeaders].valLen);
      INC(numHeaders)

    ELSIF byt >= 32 THEN
      (* 6.3: Dynamic Table Size Update — 5-bit prefix *)
      idx := DecodeInt(byt MOD 32, 5, v, pos, ok);
      IF NOT ok THEN RETURN END;
      DynResize(dt, idx)

    ELSE
      (* 6.2.2/6.2.3: Literal without indexing / never indexed *)
      idx := DecodeInt(byt MOD 16, 4, v, pos, ok);
      IF NOT ok THEN RETURN END;
      IF idx > 0 THEN
        LookupNameOnly(idx, dt, headers[numHeaders], ok);
        IF NOT ok THEN RETURN END
      ELSE
        ReadLiteralString(v, pos, headers[numHeaders].name,
                          headers[numHeaders].nameLen,
                          HpackMaxNameLen, ok);
        IF NOT ok THEN RETURN END
      END;
      ReadLiteralString(v, pos, headers[numHeaders].value,
                        headers[numHeaders].valLen,
                        HpackMaxValueLen, ok);
      IF NOT ok THEN RETURN END;
      INC(numHeaders)
    END
  END
END DecodeHeaderBlock;

(* ── Header block encode ───────────────────────────────── *)

PROCEDURE EncodeHeaderBlock(VAR b: Buf;
                            VAR dt: DynTable;
                            VAR headers: ARRAY OF HeaderEntry;
                            numHeaders: CARDINAL);
VAR i, idx, j: CARDINAL;
    exactMatch: BOOLEAN;
BEGIN
  InitStaticTable;
  i := 0;
  WHILE i < numHeaders DO
    (* Try static table *)
    idx := StaticFind(headers[i].name, headers[i].nameLen,
                      headers[i].value, headers[i].valLen, FALSE);
    exactMatch := FALSE;
    IF idx > 0 THEN
      IF EntryValEq(staticTable[idx], headers[i].value,
                    headers[i].valLen) AND
         (staticTable[idx].valLen > 0) THEN
        exactMatch := TRUE
      END
    END;

    IF exactMatch THEN
      (* Full match: indexed representation (bit 7 = 1) *)
      EncodeInt(b, idx, 7, 128)
    ELSE
      (* Literal with incremental indexing (bits 7:6 = 01) *)
      IF idx > 0 THEN
        (* Name from static table index *)
        EncodeInt(b, idx, 6, 64)
      ELSE
        (* New literal name *)
        EncodeInt(b, 0, 6, 64);
        EncodeInt(b, headers[i].nameLen, 7, 0);
        j := 0;
        WHILE j < headers[i].nameLen DO
          ByteBuf.AppendByte(b, ORD(headers[i].name[j]));
          INC(j)
        END
      END;
      (* Value *)
      EncodeInt(b, headers[i].valLen, 7, 0);
      j := 0;
      WHILE j < headers[i].valLen DO
        ByteBuf.AppendByte(b, ORD(headers[i].value[j]));
        INC(j)
      END;
      DynInsert(dt, headers[i].name, headers[i].nameLen,
                headers[i].value, headers[i].valLen)
    END;
    INC(i)
  END
END EncodeHeaderBlock;

BEGIN
  staticReady := FALSE;
  huffReady := FALSE
END Http2Hpack.
