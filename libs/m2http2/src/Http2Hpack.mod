IMPLEMENTATION MODULE Http2Hpack;

FROM Http2Types IMPORT HeaderEntry, HeaderName, HeaderValue,
                       HpackStaticTableSize, HpackMaxDynEntries,
                       HpackMaxNameLen, HpackMaxValueLen;
FROM ByteBuf IMPORT BytesView, Buf, AppendByte, ViewGetByte;

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
(* Compute 2^prefixBits - 1. *)
VAR r, i: CARDINAL;
BEGIN
  r := 1;
  i := prefixBits;
  WHILE i > 0 DO
    r := r * 2;
    DEC(i)
  END;
  RETURN r - 1
END PrefixMax;

PROCEDURE EncodeInt(VAR b: Buf; value: CARDINAL;
                    prefixBits: CARDINAL; mask: CARDINAL);
VAR maxP, v: CARDINAL;
BEGIN
  maxP := PrefixMax(prefixBits);
  IF value < maxP THEN
    AppendByte(b, mask + value)
  ELSE
    AppendByte(b, mask + maxP);
    v := value - maxP;
    WHILE v >= 128 DO
      AppendByte(b, (v MOD 128) + 128);
      v := v DIV 128
    END;
    AppendByte(b, v)
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
    byt := ViewGetByte(v, pos);
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
VAR i, nameMatch: CARDINAL;
BEGIN
  InitStaticTable;
  nameMatch := 0;
  i := 1;
  WHILE i <= HpackStaticTableSize DO
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
    a[i] := CHR(ViewGetByte(v, start + i));
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

PROCEDURE ReadLiteralString(v: BytesView; VAR pos: CARDINAL;
                            VAR arr: ARRAY OF CHAR;
                            VAR sLen: CARDINAL;
                            maxLen: CARDINAL;
                            VAR ok: BOOLEAN);
VAR byt: CARDINAL;
BEGIN
  IF pos >= v.len THEN ok := FALSE; RETURN END;
  byt := ViewGetByte(v, pos);
  INC(pos);
  (* Ignore Huffman flag (bit 7) — we don't support Huffman *)
  sLen := DecodeInt(byt MOD 128, 7, v, pos, ok);
  IF NOT ok THEN RETURN END;
  IF pos + sLen > v.len THEN ok := FALSE; RETURN END;
  IF sLen > maxLen THEN ok := FALSE; RETURN END;
  CopyViewToArr(v, pos, sLen, arr);
  INC(pos, sLen)
END ReadLiteralString;

PROCEDURE DecodeHeaderBlock(v: BytesView;
                            VAR dt: DynTable;
                            VAR headers: ARRAY OF HeaderEntry;
                            maxOut: CARDINAL;
                            VAR numHeaders: CARDINAL;
                            VAR ok: BOOLEAN);
VAR pos, byt, idx: CARDINAL;
    entry: HeaderEntry;
BEGIN
  InitStaticTable;
  ok := TRUE;
  numHeaders := 0;
  pos := 0;

  WHILE (pos < v.len) AND ok AND (numHeaders < maxOut) DO
    byt := ViewGetByte(v, pos);
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
        LookupIndex(idx, dt, entry, ok);
        IF NOT ok THEN RETURN END;
        CopyEntryName(entry, headers[numHeaders])
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
        LookupIndex(idx, dt, entry, ok);
        IF NOT ok THEN RETURN END;
        CopyEntryName(entry, headers[numHeaders])
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
          AppendByte(b, ORD(headers[i].name[j]));
          INC(j)
        END
      END;
      (* Value *)
      EncodeInt(b, headers[i].valLen, 7, 0);
      j := 0;
      WHILE j < headers[i].valLen DO
        AppendByte(b, ORD(headers[i].value[j]));
        INC(j)
      END;
      DynInsert(dt, headers[i].name, headers[i].nameLen,
                headers[i].value, headers[i].valLen)
    END;
    INC(i)
  END
END EncodeHeaderBlock;

BEGIN
  staticReady := FALSE
END Http2Hpack.
