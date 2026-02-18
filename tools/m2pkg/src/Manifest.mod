IMPLEMENTATION MODULE Manifest;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Length, Pos, Copy, Concat, CompareStr;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_fopen, m2sys_fclose, m2sys_fread_line, m2sys_fwrite_str;

CONST
  MaxDeps = 16;
  MaxExtra = 8;
  MaxCCLibs = 8;
  MaxCCExtraC = 8;
  MaxCCFrameworks = 8;
  MaxFeatures = 8;

VAR
  mName: ARRAY [0..63] OF CHAR;
  mVersion: ARRAY [0..31] OF CHAR;
  mEntry: ARRAY [0..255] OF CHAR;
  mM2plus: INTEGER;
  mIncludes: ARRAY [0..1023] OF CHAR;
  mDepNames: ARRAY [0..MaxDeps-1] OF ARRAY [0..63] OF CHAR;
  mDepPaths: ARRAY [0..MaxDeps-1] OF ARRAY [0..255] OF CHAR;
  mDepLocal: ARRAY [0..MaxDeps-1] OF INTEGER;
  mDepCount: INTEGER;
  mExtraC: ARRAY [0..MaxExtra-1] OF ARRAY [0..255] OF CHAR;
  mExtraCCount: INTEGER;
  mManifestVersion: INTEGER;
  mEdition: ARRAY [0..31] OF CHAR;
  mEditionSet: INTEGER;
  (* [cc] section *)
  mCCFlags: ARRAY [0..1023] OF CHAR;
  mLDFlags: ARRAY [0..1023] OF CHAR;
  mCCLibs: ARRAY [0..MaxCCLibs-1] OF ARRAY [0..63] OF CHAR;
  mCCLibCount: INTEGER;
  mCCExtraC: ARRAY [0..MaxCCExtraC-1] OF ARRAY [0..255] OF CHAR;
  mCCExtraCCount: INTEGER;
  mCCFrameworks: ARRAY [0..MaxCCFrameworks-1] OF ARRAY [0..63] OF CHAR;
  mCCFrameworkCount: INTEGER;
  (* [features] section *)
  mFeatureNames: ARRAY [0..MaxFeatures-1] OF ARRAY [0..63] OF CHAR;
  mFeatureDefaults: ARRAY [0..MaxFeatures-1] OF ARRAY [0..15] OF CHAR;
  mFeatureCount: INTEGER;
  (* [registry] section *)
  mRegistryURL: ARRAY [0..511] OF CHAR;

PROCEDURE Clear;
VAR i: INTEGER;
BEGIN
  mName[0] := 0C;
  mVersion[0] := 0C;
  mEntry[0] := 0C;
  mM2plus := 0;
  mIncludes[0] := 0C;
  mDepCount := 0;
  mExtraCCount := 0;
  mManifestVersion := 0;
  Assign("pim4", mEdition);
  mEditionSet := 0;
  FOR i := 0 TO MaxDeps - 1 DO
    mDepNames[i][0] := 0C;
    mDepPaths[i][0] := 0C;
    mDepLocal[i] := 0
  END;
  FOR i := 0 TO MaxExtra - 1 DO
    mExtraC[i][0] := 0C
  END;
  mCCFlags[0] := 0C;
  mLDFlags[0] := 0C;
  mCCLibCount := 0;
  mCCExtraCCount := 0;
  mCCFrameworkCount := 0;
  FOR i := 0 TO MaxCCLibs - 1 DO
    mCCLibs[i][0] := 0C
  END;
  FOR i := 0 TO MaxCCExtraC - 1 DO
    mCCExtraC[i][0] := 0C
  END;
  FOR i := 0 TO MaxCCFrameworks - 1 DO
    mCCFrameworks[i][0] := 0C
  END;
  mFeatureCount := 0;
  FOR i := 0 TO MaxFeatures - 1 DO
    mFeatureNames[i][0] := 0C;
    mFeatureDefaults[i][0] := 0C
  END;
  mRegistryURL[0] := 0C
END Clear;

(* Split space-separated value into libs (kind=0) or frameworks (kind=1) *)
PROCEDURE ParseSpaceSep(value: ARRAY OF CHAR; kind: INTEGER);
VAR
  vlen, sp, tpos, ti: INTEGER;
  tok: ARRAY [0..63] OF CHAR;
BEGIN
  vlen := Length(value);
  tpos := 0;
  WHILE tpos < vlen DO
    (* skip spaces *)
    WHILE (tpos < vlen) AND (value[tpos] = ' ') DO INC(tpos) END;
    sp := tpos;
    WHILE (sp < vlen) AND (value[sp] # ' ') DO INC(sp) END;
    IF sp > tpos THEN
      ti := 0;
      WHILE tpos < sp DO
        tok[ti] := value[tpos];
        INC(ti); INC(tpos)
      END;
      tok[ti] := 0C;
      IF kind = 0 THEN
        IF mCCLibCount < MaxCCLibs THEN
          Assign(tok, mCCLibs[mCCLibCount]);
          INC(mCCLibCount)
        END
      ELSE
        IF mCCFrameworkCount < MaxCCFrameworks THEN
          Assign(tok, mCCFrameworks[mCCFrameworkCount]);
          INC(mCCFrameworkCount)
        END
      END
    END;
    IF tpos < vlen THEN INC(tpos) END
  END
END ParseSpaceSep;

PROCEDURE ProcessLine(line: ARRAY OF CHAR; section: ARRAY OF CHAR);
VAR
  eqPos, len, vlen: INTEGER;
  key: ARRAY [0..63] OF CHAR;
  value: ARRAY [0..255] OF CHAR;
  pval: ARRAY [0..255] OF CHAR;
BEGIN
  len := Length(line);
  IF len = 0 THEN RETURN END;
  IF line[0] = '#' THEN RETURN END;

  eqPos := Pos("=", line);
  IF eqPos >= len THEN RETURN END;

  Copy(line, 0, eqPos, key);
  vlen := len - eqPos - 1;
  IF vlen > 0 THEN
    Copy(line, eqPos + 1, vlen, value)
  ELSE
    value[0] := 0C
  END;

  IF (section[0] = 'p') OR (section[0] = 0C) THEN
    IF CompareStr(key, "name") = 0 THEN
      Assign(value, mName)
    ELSIF CompareStr(key, "version") = 0 THEN
      Assign(value, mVersion)
    ELSIF CompareStr(key, "entry") = 0 THEN
      Assign(value, mEntry)
    ELSIF CompareStr(key, "m2plus") = 0 THEN
      IF (value[0] = 't') OR (value[0] = '1') THEN
        mM2plus := 1
      ELSE
        mM2plus := 0
      END
    ELSIF CompareStr(key, "includes") = 0 THEN
      Assign(value, mIncludes)
    ELSIF CompareStr(key, "extra-c") = 0 THEN
      IF mExtraCCount < MaxExtra THEN
        Assign(value, mExtraC[mExtraCCount]);
        INC(mExtraCCount)
      END
    ELSIF CompareStr(key, "manifest_version") = 0 THEN
      IF (value[0] >= '0') AND (value[0] <= '9') THEN
        mManifestVersion := ORD(value[0]) - ORD('0')
      END
    ELSIF CompareStr(key, "edition") = 0 THEN
      Assign(value, mEdition);
      mEditionSet := 1;
      IF CompareStr(value, "m2plus") = 0 THEN
        mM2plus := 1
      END
    END
  ELSIF section[0] = 'd' THEN
    IF mDepCount < MaxDeps THEN
      Assign(key, mDepNames[mDepCount]);
      IF Pos("path:", value) = 0 THEN
        Copy(value, 5, Length(value) - 5, pval);
        Assign(pval, mDepPaths[mDepCount]);
        mDepLocal[mDepCount] := 1
      ELSE
        Assign(value, mDepPaths[mDepCount]);
        mDepLocal[mDepCount] := 0
      END;
      INC(mDepCount)
    END
  ELSIF section[0] = 'c' THEN
    (* [cc] section *)
    IF CompareStr(key, "cflags") = 0 THEN
      Assign(value, mCCFlags)
    ELSIF CompareStr(key, "ldflags") = 0 THEN
      Assign(value, mLDFlags)
    ELSIF CompareStr(key, "libs") = 0 THEN
      ParseSpaceSep(value, 0)
    ELSIF CompareStr(key, "extra-c") = 0 THEN
      IF mCCExtraCCount < MaxCCExtraC THEN
        Assign(value, mCCExtraC[mCCExtraCCount]);
        INC(mCCExtraCCount)
      END
    ELSIF CompareStr(key, "frameworks") = 0 THEN
      ParseSpaceSep(value, 1)
    END
  ELSIF section[0] = 'f' THEN
    (* [features] section: key=true/false *)
    IF mFeatureCount < MaxFeatures THEN
      Assign(key, mFeatureNames[mFeatureCount]);
      Assign(value, mFeatureDefaults[mFeatureCount]);
      INC(mFeatureCount)
    END
  ELSIF section[0] = 'r' THEN
    (* [registry] section *)
    IF CompareStr(key, "url") = 0 THEN
      Assign(value, mRegistryURL)
    END
  END
END ProcessLine;

PROCEDURE Read(path: ARRAY OF CHAR): INTEGER;
VAR
  fh, n: INTEGER;
  line: ARRAY [0..511] OF CHAR;
  section: ARRAY [0..31] OF CHAR;
  rmode: ARRAY [0..1] OF CHAR;
BEGIN
  Clear;
  Assign("r", rmode);
  fh := m2sys_fopen(ADR(path), ADR(rmode));
  IF fh < 0 THEN
    RETURN -1
  END;
  Assign("package", section);
  LOOP
    n := m2sys_fread_line(fh, ADR(line), 512);
    IF n < 0 THEN EXIT END;
    IF Length(line) = 0 THEN
      (* skip blank *)
    ELSIF line[0] = '#' THEN
      (* skip comment *)
    ELSIF line[0] = '[' THEN
      IF Pos("deps", line) < Length(line) THEN
        Assign("deps", section)
      ELSIF Pos("features", line) < Length(line) THEN
        Assign("features", section)
      ELSIF Pos("registry", line) < Length(line) THEN
        Assign("registry", section)
      ELSIF Pos("cc", line) < Length(line) THEN
        Assign("cc", section)
      ELSE
        Assign("other", section)
      END
    ELSE
      ProcessLine(line, section)
    END
  END;
  n := m2sys_fclose(fh);

  (* Warn if manifest_version is missing *)
  IF mManifestVersion = 0 THEN
    WriteString("m2pkg: warning: manifest_version not set in m2.mod (assuming v1)"); WriteLn;
    mManifestVersion := 1
  END;

  (* Error if manifest_version > 1 *)
  IF mManifestVersion > 1 THEN
    WriteString("m2pkg: error: manifest_version=");
    (* Simple digit output *)
    WriteString("N");
    WriteString(" is not supported (max 1)"); WriteLn;
    RETURN -1
  END;

  (* If edition was set, it overrides m2plus *)
  IF mEditionSet = 1 THEN
    IF CompareStr(mEdition, "m2plus") = 0 THEN
      mM2plus := 1
    ELSIF CompareStr(mEdition, "pim4") = 0 THEN
      (* edition=pim4 doesn't force m2plus off if m2plus= was also set *)
    END
  END;

  RETURN 0
END Read;

PROCEDURE WrLn(fh: INTEGER);
VAR nl: ARRAY [0..1] OF CHAR;
    rc: INTEGER;
BEGIN
  nl[0] := 12C; nl[1] := 0C;
  rc := m2sys_fwrite_str(fh, ADR(nl))
END WrLn;

PROCEDURE WriteTemplate(path: ARRAY OF CHAR): INTEGER;
VAR fh, n: INTEGER;
    wmode: ARRAY [0..1] OF CHAR;
    ln: ARRAY [0..255] OF CHAR;
BEGIN
  Assign("w", wmode);
  fh := m2sys_fopen(ADR(path), ADR(wmode));
  IF fh < 0 THEN RETURN -1 END;
  Assign("# m2.mod - package manifest", ln);
  n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("manifest_version=1", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("name=mypackage", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("version=0.1.0", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("edition=pim4", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("entry=src/Main.mod", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("includes=src", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  WrLn(fh);
  Assign("[deps]", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  WrLn(fh);
  Assign("# [cc]", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("# cflags=", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("# ldflags=", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("# libs=", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("# extra-c=", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  Assign("# frameworks=", ln); n := m2sys_fwrite_str(fh, ADR(ln)); WrLn(fh);
  n := m2sys_fclose(fh);
  RETURN 0
END WriteTemplate;

PROCEDURE GetName(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mName, buf) END GetName;

PROCEDURE GetVersion(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mVersion, buf) END GetVersion;

PROCEDURE GetEntry(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mEntry, buf) END GetEntry;

PROCEDURE IsM2Plus(): INTEGER;
BEGIN RETURN mM2plus END IsM2Plus;

PROCEDURE GetIncludes(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mIncludes, buf) END GetIncludes;

PROCEDURE DepCount(): INTEGER;
BEGIN RETURN mDepCount END DepCount;

PROCEDURE GetDepName(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mDepCount) THEN
    Assign(mDepNames[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetDepName;

PROCEDURE GetDepPath(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mDepCount) THEN
    Assign(mDepPaths[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetDepPath;

PROCEDURE IsDepLocal(i: INTEGER): INTEGER;
BEGIN
  IF (i >= 0) AND (i < mDepCount) THEN
    RETURN mDepLocal[i]
  ELSE
    RETURN 0
  END
END IsDepLocal;

PROCEDURE ExtraCCount(): INTEGER;
BEGIN RETURN mExtraCCount END ExtraCCount;

PROCEDURE GetExtraC(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mExtraCCount) THEN
    Assign(mExtraC[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetExtraC;

PROCEDURE GetManifestVersion(): INTEGER;
BEGIN RETURN mManifestVersion END GetManifestVersion;

PROCEDURE GetEdition(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mEdition, buf) END GetEdition;

PROCEDURE GetCCFlags(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mCCFlags, buf) END GetCCFlags;

PROCEDURE GetLDFlags(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mLDFlags, buf) END GetLDFlags;

PROCEDURE CCLibCount(): INTEGER;
BEGIN RETURN mCCLibCount END CCLibCount;

PROCEDURE GetCCLib(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mCCLibCount) THEN
    Assign(mCCLibs[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetCCLib;

PROCEDURE CCExtraCCount(): INTEGER;
BEGIN RETURN mCCExtraCCount END CCExtraCCount;

PROCEDURE GetCCExtraC(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mCCExtraCCount) THEN
    Assign(mCCExtraC[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetCCExtraC;

PROCEDURE CCFrameworkCount(): INTEGER;
BEGIN RETURN mCCFrameworkCount END CCFrameworkCount;

PROCEDURE GetCCFramework(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mCCFrameworkCount) THEN
    Assign(mCCFrameworks[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetCCFramework;

PROCEDURE GetDepVersion(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  (* For non-local deps, mDepPaths[i] holds the version string *)
  IF (i >= 0) AND (i < mDepCount) AND (mDepLocal[i] = 0) THEN
    Assign(mDepPaths[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetDepVersion;

PROCEDURE FeatureCount(): INTEGER;
BEGIN RETURN mFeatureCount END FeatureCount;

PROCEDURE GetFeatureName(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mFeatureCount) THEN
    Assign(mFeatureNames[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetFeatureName;

PROCEDURE GetFeatureDefault(i: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  IF (i >= 0) AND (i < mFeatureCount) THEN
    Assign(mFeatureDefaults[i], buf)
  ELSE
    buf[0] := 0C
  END
END GetFeatureDefault;

PROCEDURE IsFeatureEnabled(name: ARRAY OF CHAR): INTEGER;
VAR j: INTEGER;
BEGIN
  FOR j := 0 TO mFeatureCount - 1 DO
    IF CompareStr(mFeatureNames[j], name) = 0 THEN
      IF (mFeatureDefaults[j][0] = 't') OR (mFeatureDefaults[j][0] = '1') THEN
        RETURN 1
      ELSE
        RETURN 0
      END
    END
  END;
  RETURN 0
END IsFeatureEnabled;

PROCEDURE GetRegistryURL(VAR buf: ARRAY OF CHAR);
BEGIN Assign(mRegistryURL, buf) END GetRegistryURL;

PROCEDURE SetRegistryURL(url: ARRAY OF CHAR);
BEGIN Assign(url, mRegistryURL) END SetRegistryURL;

END Manifest.
