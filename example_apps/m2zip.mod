MODULE m2zip;
(* ================================================================
   m2zip - A ZIP archive utility written in Modula-2

   Supports:
     m2zip c archive.zip file1 file2 ...   -- create archive
     m2zip l archive.zip                   -- list contents
     m2zip x archive.zip                   -- extract all files

   Uses STORE method (no compression) with CRC-32 checksums.
   Implements the PKZIP format: local headers, central directory,
   and end-of-central-directory record.
   ================================================================ *)

FROM InOut IMPORT WriteString, WriteInt, WriteCard, WriteLn, WriteHex;
FROM Args IMPORT ArgCount, GetArg;
FROM BinaryIO IMPORT OpenRead, OpenWrite, Close, ReadByte, WriteByte,
                      ReadBytes, WriteBytes, FileSize, Seek, Tell, Done;

CONST
  MaxFiles     = 64;
  MaxNameLen   = 255;
  BufSize      = 4096;

  (* ZIP signatures *)
  LocalSig1    = 050h;   (* 'P' *)
  LocalSig2    = 04Bh;   (* 'K' *)
  LocalSig3    = 003h;
  LocalSig4    = 004h;

  CentralSig3  = 001h;
  CentralSig4  = 002h;

  EndSig3      = 005h;
  EndSig4      = 006h;

TYPE
  FileName = ARRAY [0..MaxNameLen] OF CHAR;
  Buffer   = ARRAY [0..BufSize-1] OF CHAR;

  (* Info about one file entry in the archive *)
  FileEntry = RECORD
    name:       FileName;
    nameLen:    CARDINAL;
    crc32:      CARDINAL;
    size:       CARDINAL;
    offset:     CARDINAL;   (* offset of local header in archive *)
  END;

VAR
  command:    ARRAY [0..1] OF CHAR;
  archName:  ARRAY [0..255] OF CHAR;
  entries:   ARRAY [0..MaxFiles-1] OF FileEntry;
  numEntries: CARDINAL;

  (* CRC-32 lookup table *)
  crcTable:  ARRAY [0..255] OF CARDINAL;
  crcInited: BOOLEAN;

(* ── CRC-32 ────────────────────────────────────────────────────── *)

PROCEDURE InitCRC;
VAR
  i, j, c: CARDINAL;
BEGIN
  FOR i := 0 TO 255 DO
    c := i;
    FOR j := 0 TO 7 DO
      IF ODD(c) THEN
        c := BXOR(SHR(c, 1), 3988292384)  (* 0xEDB88320 *)
      ELSE
        c := SHR(c, 1)
      END
    END;
    crcTable[i] := c
  END;
  crcInited := TRUE
END InitCRC;

PROCEDURE UpdateCRC(crc: CARDINAL; b: CARDINAL): CARDINAL;
VAR idx: CARDINAL;
BEGIN
  idx := BAND(BXOR(crc, b), 255);
  RETURN BXOR(SHR(crc, 8), crcTable[idx])
END UpdateCRC;

PROCEDURE CRCofFile(fh: CARDINAL; size: CARDINAL): CARDINAL;
VAR
  crc, b, i: CARDINAL;
BEGIN
  crc := 0FFFFFFFFh;
  FOR i := 1 TO size DO
    ReadByte(fh, b);
    crc := UpdateCRC(crc, b)
  END;
  RETURN BXOR(crc, 0FFFFFFFFh)
END CRCofFile;

(* ── String helpers ────────────────────────────────────────────── *)

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    INC(i)
  END;
  RETURN i
END StrLen;

PROCEDURE StrEqual(VAR a, b: ARRAY OF CHAR): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  i := 0;
  LOOP
    IF (i > HIGH(a)) OR (a[i] = 0C) THEN
      RETURN (i > HIGH(b)) OR (b[i] = 0C)
    END;
    IF (i > HIGH(b)) OR (b[i] = 0C) THEN
      RETURN FALSE
    END;
    IF a[i] # b[i] THEN
      RETURN FALSE
    END;
    INC(i)
  END
END StrEqual;

(* ── Binary write helpers (little-endian) ──────────────────────── *)

PROCEDURE Write16(fh: CARDINAL; val: CARDINAL);
BEGIN
  WriteByte(fh, val MOD 256);
  WriteByte(fh, (val DIV 256) MOD 256)
END Write16;

PROCEDURE Write32(fh: CARDINAL; val: CARDINAL);
BEGIN
  WriteByte(fh, val MOD 256);
  WriteByte(fh, (val DIV 256) MOD 256);
  WriteByte(fh, (val DIV 65536) MOD 256);
  WriteByte(fh, (val DIV 16777216) MOD 256)
END Write32;

PROCEDURE Read16(fh: CARDINAL; VAR val: CARDINAL);
VAR lo, hi: CARDINAL;
BEGIN
  ReadByte(fh, lo);
  ReadByte(fh, hi);
  val := lo + hi * 256
END Read16;

PROCEDURE Read32(fh: CARDINAL; VAR val: CARDINAL);
VAR b0, b1, b2, b3: CARDINAL;
BEGIN
  ReadByte(fh, b0);
  ReadByte(fh, b1);
  ReadByte(fh, b2);
  ReadByte(fh, b3);
  val := b0 + b1*256 + b2*65536 + b3*16777216
END Read32;

PROCEDURE WriteNameBytes(fh: CARDINAL; VAR name: ARRAY OF CHAR; len: CARDINAL);
VAR i: CARDINAL;
BEGIN
  FOR i := 0 TO len-1 DO
    WriteByte(fh, ORD(name[i]))
  END
END WriteNameBytes;

(* ── Write ZIP local file header ───────────────────────────────── *)

PROCEDURE WriteLocalHeader(fh: CARDINAL; VAR e: FileEntry);
BEGIN
  (* Signature: PK\003\004 *)
  WriteByte(fh, LocalSig1);
  WriteByte(fh, LocalSig2);
  WriteByte(fh, LocalSig3);
  WriteByte(fh, LocalSig4);
  (* Version needed to extract: 2.0 *)
  Write16(fh, 20);
  (* General purpose bit flag *)
  Write16(fh, 0);
  (* Compression method: 0 = stored *)
  Write16(fh, 0);
  (* Last mod file time *)
  Write16(fh, 0);
  (* Last mod file date *)
  Write16(fh, 0);
  (* CRC-32 *)
  Write32(fh, e.crc32);
  (* Compressed size (= uncompressed for stored) *)
  Write32(fh, e.size);
  (* Uncompressed size *)
  Write32(fh, e.size);
  (* File name length *)
  Write16(fh, e.nameLen);
  (* Extra field length *)
  Write16(fh, 0);
  (* File name *)
  WriteNameBytes(fh, e.name, e.nameLen)
END WriteLocalHeader;

(* ── Write ZIP central directory header ────────────────────────── *)

PROCEDURE WriteCentralHeader(fh: CARDINAL; VAR e: FileEntry);
BEGIN
  (* Signature: PK\001\002 *)
  WriteByte(fh, LocalSig1);
  WriteByte(fh, LocalSig2);
  WriteByte(fh, CentralSig3);
  WriteByte(fh, CentralSig4);
  (* Version made by: 2.0 *)
  Write16(fh, 20);
  (* Version needed: 2.0 *)
  Write16(fh, 20);
  (* Flags *)
  Write16(fh, 0);
  (* Compression method: stored *)
  Write16(fh, 0);
  (* Mod time *)
  Write16(fh, 0);
  (* Mod date *)
  Write16(fh, 0);
  (* CRC-32 *)
  Write32(fh, e.crc32);
  (* Compressed size *)
  Write32(fh, e.size);
  (* Uncompressed size *)
  Write32(fh, e.size);
  (* File name length *)
  Write16(fh, e.nameLen);
  (* Extra field length *)
  Write16(fh, 0);
  (* File comment length *)
  Write16(fh, 0);
  (* Disk number start *)
  Write16(fh, 0);
  (* Internal attributes *)
  Write16(fh, 0);
  (* External attributes *)
  Write32(fh, 0);
  (* Relative offset of local header *)
  Write32(fh, e.offset);
  (* File name *)
  WriteNameBytes(fh, e.name, e.nameLen)
END WriteCentralHeader;

(* ── Write end-of-central-directory record ────────────────────── *)

PROCEDURE WriteEndRecord(fh: CARDINAL; numEnt, cdirSize, cdirOffset: CARDINAL);
BEGIN
  (* Signature: PK\005\006 *)
  WriteByte(fh, LocalSig1);
  WriteByte(fh, LocalSig2);
  WriteByte(fh, EndSig3);
  WriteByte(fh, EndSig4);
  (* Number of this disk *)
  Write16(fh, 0);
  (* Disk where central dir starts *)
  Write16(fh, 0);
  (* Number of entries on this disk *)
  Write16(fh, numEnt);
  (* Total number of entries *)
  Write16(fh, numEnt);
  (* Size of central directory *)
  Write32(fh, cdirSize);
  (* Offset of central directory *)
  Write32(fh, cdirOffset);
  (* ZIP comment length *)
  Write16(fh, 0)
END WriteEndRecord;

(* ── Copy file data from source to archive ─────────────────────── *)

PROCEDURE CopyFileData(src, dst: CARDINAL; size: CARDINAL);
VAR
  b, i: CARDINAL;
BEGIN
  FOR i := 1 TO size DO
    ReadByte(src, b);
    WriteByte(dst, b)
  END
END CopyFileData;

(* ── CREATE command ────────────────────────────────────────────── *)

PROCEDURE DoCreate;
VAR
  argIdx, nArgs: CARDINAL;
  fname: FileName;
  inFh, outFh: CARDINAL;
  fsize: CARDINAL;
  cdirStart, cdirEnd: CARDINAL;
  i: CARDINAL;
BEGIN
  nArgs := ArgCount();
  IF nArgs < 4 THEN
    WriteString("Usage: m2zip c archive.zip file1 [file2 ...]"); WriteLn;
    RETURN
  END;

  IF NOT crcInited THEN InitCRC END;

  numEntries := 0;

  (* First pass: gather file info (size + CRC) *)
  FOR argIdx := 3 TO nArgs - 1 DO
    GetArg(argIdx, fname);
    OpenRead(fname, inFh);
    IF NOT Done THEN
      WriteString("Error: cannot open "); WriteString(fname); WriteLn;
      RETURN
    END;
    FileSize(inFh, fsize);

    entries[numEntries].nameLen := StrLen(fname);
    FOR i := 0 TO entries[numEntries].nameLen - 1 DO
      entries[numEntries].name[i] := fname[i]
    END;
    entries[numEntries].name[entries[numEntries].nameLen] := 0C;
    entries[numEntries].size := fsize;

    (* Compute CRC *)
    Seek(inFh, 0);
    entries[numEntries].crc32 := CRCofFile(inFh, fsize);
    Close(inFh);

    INC(numEntries)
  END;

  (* Open output archive *)
  GetArg(2, archName);
  OpenWrite(archName, outFh);
  IF NOT Done THEN
    WriteString("Error: cannot create "); WriteString(archName); WriteLn;
    RETURN
  END;

  (* Write local headers + file data *)
  FOR i := 0 TO numEntries - 1 DO
    Tell(outFh, entries[i].offset);
    WriteLocalHeader(outFh, entries[i]);

    (* Copy file data *)
    GetArg(i + 3, fname);
    OpenRead(fname, inFh);
    CopyFileData(inFh, outFh, entries[i].size);
    Close(inFh);

    WriteString("  adding: ");
    WriteString(entries[i].name);
    WriteString(" (");
    WriteCard(entries[i].size, 1);
    WriteString(" bytes)");
    WriteLn
  END;

  (* Write central directory *)
  Tell(outFh, cdirStart);
  FOR i := 0 TO numEntries - 1 DO
    WriteCentralHeader(outFh, entries[i])
  END;
  Tell(outFh, cdirEnd);

  (* Write end of central directory *)
  WriteEndRecord(outFh, numEntries, cdirEnd - cdirStart, cdirStart);
  Close(outFh);

  WriteString("Created ");
  WriteString(archName);
  WriteString(" with ");
  WriteCard(numEntries, 1);
  WriteString(" file(s)");
  WriteLn
END DoCreate;

(* ── LIST command ──────────────────────────────────────────────── *)

PROCEDURE DoList;
VAR
  fh: CARDINAL;
  sig1, sig2, sig3, sig4: CARDINAL;
  version, flags, method: CARDINAL;
  modTime, modDate: CARDINAL;
  crc, compSize, uncompSize: CARDINAL;
  nameLen, extraLen: CARDINAL;
  i: CARDINAL;
  ch: CARDINAL;
  name: FileName;
  totalSize, totalFiles: CARDINAL;
BEGIN
  GetArg(2, archName);
  OpenRead(archName, fh);
  IF NOT Done THEN
    WriteString("Error: cannot open "); WriteString(archName); WriteLn;
    RETURN
  END;

  WriteString("Archive: "); WriteString(archName); WriteLn;
  WriteString("  Length     CRC-32     Name"); WriteLn;
  WriteString("  ------     ------     ----"); WriteLn;

  totalSize := 0;
  totalFiles := 0;

  LOOP
    (* Read signature *)
    ReadByte(fh, sig1);
    IF NOT Done THEN EXIT END;
    ReadByte(fh, sig2);
    ReadByte(fh, sig3);
    ReadByte(fh, sig4);

    IF (sig1 = LocalSig1) AND (sig2 = LocalSig2) AND
       (sig3 = LocalSig3) AND (sig4 = LocalSig4) THEN
      (* Local file header *)
      Read16(fh, version);
      Read16(fh, flags);
      Read16(fh, method);
      Read16(fh, modTime);
      Read16(fh, modDate);
      Read32(fh, crc);
      Read32(fh, compSize);
      Read32(fh, uncompSize);
      Read16(fh, nameLen);
      Read16(fh, extraLen);

      (* Read file name *)
      FOR i := 0 TO nameLen - 1 DO
        ReadByte(fh, ch);
        IF i <= MaxNameLen THEN
          name[i] := CHR(ch)
        END
      END;
      IF nameLen <= MaxNameLen THEN
        name[nameLen] := 0C
      ELSE
        name[MaxNameLen] := 0C
      END;

      (* Skip extra field *)
      FOR i := 1 TO extraLen DO
        ReadByte(fh, ch)
      END;

      (* Display entry *)
      WriteCard(uncompSize, 8);
      WriteString("     ");
      WriteHex(crc, 8);
      WriteString("     ");
      WriteString(name);
      WriteLn;

      totalSize := totalSize + uncompSize;
      INC(totalFiles);

      (* Skip file data *)
      FOR i := 1 TO compSize DO
        ReadByte(fh, ch)
      END
    ELSE
      (* Not a local file header - done with local entries *)
      EXIT
    END
  END;

  WriteString("  ------                ----"); WriteLn;
  WriteCard(totalSize, 8);
  WriteString("     ");
  WriteCard(totalFiles, 1);
  WriteString(" file(s)");
  WriteLn;

  Close(fh)
END DoList;

(* ── EXTRACT command ───────────────────────────────────────────── *)

PROCEDURE DoExtract;
VAR
  fh, outFh: CARDINAL;
  sig1, sig2, sig3, sig4: CARDINAL;
  version, flags, method: CARDINAL;
  modTime, modDate: CARDINAL;
  crc, compSize, uncompSize: CARDINAL;
  nameLen, extraLen: CARDINAL;
  i: CARDINAL;
  ch, b: CARDINAL;
  name: FileName;
  verifyCrc: CARDINAL;
BEGIN
  IF NOT crcInited THEN InitCRC END;

  GetArg(2, archName);
  OpenRead(archName, fh);
  IF NOT Done THEN
    WriteString("Error: cannot open "); WriteString(archName); WriteLn;
    RETURN
  END;

  LOOP
    ReadByte(fh, sig1);
    IF NOT Done THEN EXIT END;
    ReadByte(fh, sig2);
    ReadByte(fh, sig3);
    ReadByte(fh, sig4);

    IF (sig1 = LocalSig1) AND (sig2 = LocalSig2) AND
       (sig3 = LocalSig3) AND (sig4 = LocalSig4) THEN
      Read16(fh, version);
      Read16(fh, flags);
      Read16(fh, method);
      Read16(fh, modTime);
      Read16(fh, modDate);
      Read32(fh, crc);
      Read32(fh, compSize);
      Read32(fh, uncompSize);
      Read16(fh, nameLen);
      Read16(fh, extraLen);

      FOR i := 0 TO nameLen - 1 DO
        ReadByte(fh, ch);
        IF i <= MaxNameLen THEN
          name[i] := CHR(ch)
        END
      END;
      IF nameLen <= MaxNameLen THEN
        name[nameLen] := 0C
      ELSE
        name[MaxNameLen] := 0C
      END;

      FOR i := 1 TO extraLen DO
        ReadByte(fh, ch)
      END;

      IF method # 0 THEN
        WriteString("  skipping: ");
        WriteString(name);
        WriteString(" (compressed, method=");
        WriteCard(method, 1);
        WriteString(")");
        WriteLn;
        (* Skip data *)
        FOR i := 1 TO compSize DO
          ReadByte(fh, ch)
        END
      ELSE
        (* Extract stored file *)
        WriteString("  extracting: ");
        WriteString(name);

        OpenWrite(name, outFh);
        IF NOT Done THEN
          WriteString(" - ERROR: cannot create file"); WriteLn;
          FOR i := 1 TO compSize DO
            ReadByte(fh, ch)
          END
        ELSE
          (* Extract and compute CRC *)
          verifyCrc := 0FFFFFFFFh;
          FOR i := 1 TO uncompSize DO
            ReadByte(fh, b);
            verifyCrc := UpdateCRC(verifyCrc, b);
            WriteByte(outFh, b)
          END;
          Close(outFh);
          verifyCrc := BXOR(verifyCrc, 0FFFFFFFFh);

          IF verifyCrc = crc THEN
            WriteString(" OK")
          ELSE
            WriteString(" CRC MISMATCH!")
          END;
          WriteString(" (");
          WriteCard(uncompSize, 1);
          WriteString(" bytes)");
          WriteLn
        END
      END
    ELSE
      EXIT
    END
  END;

  Close(fh);
  WriteString("Done"); WriteLn
END DoExtract;

(* ── Main ──────────────────────────────────────────────────────── *)

PROCEDURE ShowUsage;
BEGIN
  WriteString("m2zip - Modula-2 ZIP archive utility"); WriteLn;
  WriteLn;
  WriteString("Usage:"); WriteLn;
  WriteString("  m2zip c archive.zip file1 [file2 ...]  -- create archive"); WriteLn;
  WriteString("  m2zip l archive.zip                    -- list contents"); WriteLn;
  WriteString("  m2zip x archive.zip                    -- extract all"); WriteLn
END ShowUsage;

BEGIN
  crcInited := FALSE;
  numEntries := 0;

  IF ArgCount() < 3 THEN
    ShowUsage;
    RETURN
  END;

  GetArg(1, command);

  IF command[0] = "c" THEN
    DoCreate
  ELSIF command[0] = "l" THEN
    DoList
  ELSIF command[0] = "x" THEN
    DoExtract
  ELSE
    ShowUsage
  END
END m2zip.
