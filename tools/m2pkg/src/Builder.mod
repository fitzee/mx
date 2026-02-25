IMPLEMENTATION MODULE Builder;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Concat, Length, CompareStr;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Sys IMPORT m2sys_exec, m2sys_file_exists, m2sys_getcwd,
               m2sys_join_path, m2sys_mkdir_p, m2sys_basename;
FROM Manifest IMPORT GetEntry, IsM2Plus, GetIncludes, DepCount, GetDepPath,
                     ExtraCCount, GetExtraC, GetName, IsDepLocal,
                     GetCCFlags, GetLDFlags, CCLibCount, GetCCLib,
                     CCExtraCCount, GetCCExtraC, CCFrameworkCount, GetCCFramework,
                     Read, Clear, GetDepName,
                     FeatureCount, GetFeatureName, IsFeatureEnabled;
FROM Lockfile IMPORT LockDepCount, GetDepResolvedPath, GetDepLockName,
                     Exists;
IMPORT Lockfile;

VAR
  cmd: ARRAY [0..4095] OF CHAR;
  tmp: ARRAY [0..4095] OF CHAR;

PROCEDURE Append(s: ARRAY OF CHAR);
BEGIN
  Concat(cmd, s, tmp);
  Assign(tmp, cmd)
END Append;

(* Split space-separated cflags and emit --cflag for each *)
PROCEDURE AppendCFlags(flags: ARRAY OF CHAR);
VAR
  p, s, fi, flen: INTEGER;
  flag: ARRAY [0..255] OF CHAR;
BEGIN
  flen := Length(flags);
  p := 0;
  WHILE p < flen DO
    WHILE (p < flen) AND (flags[p] = ' ') DO INC(p) END;
    s := p;
    WHILE (p < flen) AND (flags[p] # ' ') DO INC(p) END;
    IF p > s THEN
      fi := 0;
      WHILE s < p DO
        flag[fi] := flags[s];
        INC(fi); INC(s)
      END;
      flag[fi] := 0C;
      Append(" --cflag ");
      Append(flag)
    END
  END
END AppendCFlags;

(* Append transitive [cc] flags from all local dependencies *)
PROCEDURE AppendDepCC;
VAR
  i, j, k, ndeps2, nlibs, nfws, nexcc: INTEGER;
  depPath2: ARRAY [0..255] OF CHAR;
  depManifest: ARRAY [0..511] OF CHAR;
  ccBuf: ARRAY [0..1023] OF CHAR;
  libBuf: ARRAY [0..63] OF CHAR;
  fwBuf: ARRAY [0..63] OF CHAR;
  ecBuf: ARRAY [0..255] OF CHAR;
  savedPaths: ARRAY [0..15] OF ARRAY [0..255] OF CHAR;
  savedLocal: ARRAY [0..15] OF INTEGER;
  savedNames: ARRAY [0..15] OF ARRAY [0..63] OF CHAR;
  lockName: ARRAY [0..63] OF CHAR;
  seenEC: ARRAY [0..31] OF ARRAY [0..63] OF CHAR;
  nseenEC: INTEGER;
  ecBase: ARRAY [0..63] OF CHAR;
  seenLibs: ARRAY [0..15] OF ARRAY [0..63] OF CHAR;
  nseenLibs: INTEGER;
  isDup: INTEGER;
  transPaths: ARRAY [0..63] OF ARRAY [0..511] OF CHAR;
  transNames: ARRAY [0..63] OF ARRAY [0..63] OF CHAR;
  ntrans, nDirectLocal, bfsIdx, ndeps3, k2, k3: INTEGER;
  transDepPath: ARRAY [0..255] OF CHAR;
  transResolved: ARRAY [0..511] OF CHAR;
  transInc: ARRAY [0..511] OF CHAR;
  transManifest: ARRAY [0..511] OF CHAR;
  transBase: ARRAY [0..63] OF CHAR;
BEGIN
  ndeps2 := DepCount();
  (* Seed seen lists with main manifest's extra-c and libs basenames
     so transitive deps don't duplicate them *)
  nseenEC := 0;
  i := 0;
  WHILE i < ExtraCCount() DO
    GetExtraC(i, ecBuf);
    m2sys_basename(ADR(ecBuf), ADR(seenEC[nseenEC]), 64);
    INC(nseenEC);
    INC(i)
  END;
  i := 0;
  WHILE i < CCExtraCCount() DO
    GetCCExtraC(i, ecBuf);
    m2sys_basename(ADR(ecBuf), ADR(seenEC[nseenEC]), 64);
    INC(nseenEC);
    INC(i)
  END;
  nseenLibs := 0;
  i := 0;
  WHILE i < CCLibCount() DO
    GetCCLib(i, libBuf);
    IF nseenLibs < 16 THEN
      Assign(libBuf, seenLibs[nseenLibs]);
      INC(nseenLibs)
    END;
    INC(i)
  END;
  (* Snapshot dep paths and local flags before the loop,
     because Clear/Read inside the loop overwrites Manifest state *)
  j := 0;
  WHILE j < ndeps2 DO
    GetDepPath(j, savedPaths[j]);
    savedLocal[j] := IsDepLocal(j);
    GetDepName(j, savedNames[j]);
    INC(j)
  END;
  j := 0;
  WHILE j < ndeps2 DO
    IF savedLocal[j] = 1 THEN
      Assign(savedPaths[j], depPath2)
    ELSE
      (* Remote dep — resolve path from lockfile *)
      depPath2[0] := 0C;
      k2 := 0;
      WHILE k2 < LockDepCount() DO
        GetDepLockName(k2, transBase);
        IF CompareStr(transBase, savedNames[j]) = 0 THEN
          GetDepResolvedPath(k2, depPath2);
          k2 := LockDepCount()
        END;
        INC(k2)
      END
    END;
    IF Length(depPath2) > 0 THEN
      (* Build path to dep's m2.toml *)
      Assign(depPath2, depManifest);
      Concat(depManifest, "/m2.toml", tmp);
      Assign(tmp, depManifest);
      (* Save current manifest state, read dep manifest *)
      Clear;
      Read(depManifest);
      (* Append dep's cflags via --cflag *)
      GetCCFlags(ccBuf);
      IF Length(ccBuf) > 0 THEN
        AppendCFlags(ccBuf)
      END;
      (* Append dep's libs, skipping duplicates *)
      nlibs := CCLibCount();
      i := 0;
      WHILE i < nlibs DO
        GetCCLib(i, libBuf);
        isDup := 0;
        k := 0;
        WHILE k < nseenLibs DO
          IF CompareStr(libBuf, seenLibs[k]) = 0 THEN isDup := 1 END;
          INC(k)
        END;
        IF isDup = 0 THEN
          Append(" ");
          Append(libBuf);
          IF nseenLibs < 16 THEN
            Assign(libBuf, seenLibs[nseenLibs]);
            INC(nseenLibs)
          END
        END;
        INC(i)
      END;
      (* Append dep's ldflags *)
      GetLDFlags(ccBuf);
      IF Length(ccBuf) > 0 THEN
        Append(" ");
        Append(ccBuf)
      END;
      (* Append dep's extra-c with dep path prefix, skipping duplicates *)
      nexcc := CCExtraCCount();
      i := 0;
      WHILE i < nexcc DO
        GetCCExtraC(i, ecBuf);
        m2sys_basename(ADR(ecBuf), ADR(ecBase), 64);
        isDup := 0;
        k := 0;
        WHILE k < nseenEC DO
          IF CompareStr(ecBase, seenEC[k]) = 0 THEN isDup := 1 END;
          INC(k)
        END;
        IF isDup = 0 THEN
          Append(" ");
          Append(depPath2);
          Append("/");
          Append(ecBuf);
          IF nseenEC < 32 THEN
            Assign(ecBase, seenEC[nseenEC]);
            INC(nseenEC)
          END
        END;
        INC(i)
      END;
      (* Append dep's frameworks *)
      nfws := CCFrameworkCount();
      i := 0;
      WHILE i < nfws DO
        GetCCFramework(i, fwBuf);
        Append(" -framework ");
        Append(fwBuf);
        INC(i)
      END
    END;
    INC(j)
  END;

  (* --- Transitive dependency resolution (BFS) --- *)
  (* Seed BFS queue with ALL direct deps (local + remote via lockfile) *)
  ntrans := 0;
  j := 0;
  WHILE j < ndeps2 DO
    IF savedLocal[j] = 1 THEN
      Assign(savedPaths[j], transPaths[ntrans]);
    ELSE
      (* Remote dep — look up in lockfile by name *)
      transPaths[ntrans][0] := 0C;
      k2 := 0;
      WHILE k2 < LockDepCount() DO
        GetDepLockName(k2, transBase);
        IF CompareStr(transBase, savedNames[j]) = 0 THEN
          GetDepResolvedPath(k2, transPaths[ntrans]);
          k2 := LockDepCount()  (* break *)
        END;
        INC(k2)
      END
    END;
    IF Length(transPaths[ntrans]) > 0 THEN
      m2sys_basename(ADR(transPaths[ntrans]), ADR(transNames[ntrans]), 64);
      INC(ntrans)
    END;
    INC(j)
  END;
  nDirectLocal := ntrans;

  (* BFS: for each dep, read its manifest and discover its deps *)
  bfsIdx := 0;
  WHILE bfsIdx < ntrans DO
    Assign(transPaths[bfsIdx], transManifest);
    Concat(transManifest, "/m2.toml", tmp);
    Assign(tmp, transManifest);
    Clear;
    Read(transManifest);
    ndeps3 := DepCount();
    k2 := 0;
    WHILE k2 < ndeps3 DO
      IF IsDepLocal(k2) = 1 THEN
        GetDepPath(k2, transDepPath);
        m2sys_join_path(ADR(transPaths[bfsIdx]), ADR(transDepPath),
                        ADR(transResolved), 512);
      ELSE
        (* Non-local dep — look up in lockfile by name *)
        GetDepName(k2, transBase);
        transResolved[0] := 0C;
        k3 := 0;
        WHILE k3 < LockDepCount() DO
          GetDepLockName(k3, lockName);
          IF CompareStr(transBase, lockName) = 0 THEN
            GetDepResolvedPath(k3, transResolved);
            k3 := LockDepCount()  (* break *)
          END;
          INC(k3)
        END
      END;
      IF Length(transResolved) > 0 THEN
        m2sys_basename(ADR(transResolved), ADR(transBase), 64);
        (* Deduplicate by basename *)
        isDup := 0;
        k3 := 0;
        WHILE k3 < ntrans DO
          IF CompareStr(transBase, transNames[k3]) = 0 THEN isDup := 1 END;
          INC(k3)
        END;
        IF (isDup = 0) AND (ntrans < 64) THEN
          Assign(transResolved, transPaths[ntrans]);
          Assign(transBase, transNames[ntrans]);
          INC(ntrans)
        END
      END;
      INC(k2)
    END;
    INC(bfsIdx)
  END;

  (* Add -I for transitive deps (direct deps already handled in Build) *)
  k2 := nDirectLocal;
  WHILE k2 < ntrans DO
    m2sys_join_path(ADR(transPaths[k2]), ADR("src"), ADR(transInc), 512);
    Append(" -I ");
    Append(transInc);
    INC(k2)
  END;

  (* Process [cc] flags for transitive deps *)
  k2 := nDirectLocal;
  WHILE k2 < ntrans DO
    Assign(transPaths[k2], transManifest);
    Concat(transManifest, "/m2.toml", tmp);
    Assign(tmp, transManifest);
    Clear;
    Read(transManifest);
    (* cflags *)
    GetCCFlags(ccBuf);
    IF Length(ccBuf) > 0 THEN
      AppendCFlags(ccBuf)
    END;
    (* libs with dedup *)
    nlibs := CCLibCount();
    i := 0;
    WHILE i < nlibs DO
      GetCCLib(i, libBuf);
      isDup := 0;
      k := 0;
      WHILE k < nseenLibs DO
        IF CompareStr(libBuf, seenLibs[k]) = 0 THEN isDup := 1 END;
        INC(k)
      END;
      IF isDup = 0 THEN
        Append(" ");
        Append(libBuf);
        IF nseenLibs < 16 THEN
          Assign(libBuf, seenLibs[nseenLibs]);
          INC(nseenLibs)
        END
      END;
      INC(i)
    END;
    (* ldflags *)
    GetLDFlags(ccBuf);
    IF Length(ccBuf) > 0 THEN
      Append(" ");
      Append(ccBuf)
    END;
    (* extra-c with dep path prefix and dedup *)
    nexcc := CCExtraCCount();
    i := 0;
    WHILE i < nexcc DO
      GetCCExtraC(i, ecBuf);
      m2sys_basename(ADR(ecBuf), ADR(ecBase), 64);
      isDup := 0;
      k := 0;
      WHILE k < nseenEC DO
        IF CompareStr(ecBase, seenEC[k]) = 0 THEN isDup := 1 END;
        INC(k)
      END;
      IF isDup = 0 THEN
        Append(" ");
        Append(transPaths[k2]);
        Append("/");
        Append(ecBuf);
        IF nseenEC < 32 THEN
          Assign(ecBase, seenEC[nseenEC]);
          INC(nseenEC)
        END
      END;
      INC(i)
    END;
    (* frameworks *)
    nfws := CCFrameworkCount();
    i := 0;
    WHILE i < nfws DO
      GetCCFramework(i, fwBuf);
      Append(" -framework ");
      Append(fwBuf);
      INC(i)
    END;
    INC(k2)
  END;

  (* Re-read main manifest to restore state *)
  Clear;
  Read("m2.toml")
END AppendDepCC;

PROCEDURE Build(release: INTEGER; target: ARRAY OF CHAR);
VAR
  rc, i, ndeps, nextra: INTEGER;
  entry: ARRAY [0..255] OF CHAR;
  incl: ARRAY [0..1023] OF CHAR;
  depPath: ARRAY [0..255] OF CHAR;
  extraFile: ARRAY [0..255] OF CHAR;
  name: ARRAY [0..63] OF CHAR;
  outPath: ARRAY [0..511] OF CHAR;
  outDir: ARRAY [0..255] OF CHAR;
  depSrc: ARRAY [0..511] OF CHAR;
  tok: ARRAY [0..255] OF CHAR;
  tpos, tlen, sp: INTEGER;
  lockPath: ARRAY [0..15] OF CHAR;
BEGIN
  (* Read lockfile if it exists for resolved paths *)
  Assign("m2.lock", lockPath);
  IF m2sys_file_exists(ADR(lockPath)) = 1 THEN
    Lockfile.Read("m2.lock")
  END;

  GetEntry(entry);
  IF Length(entry) = 0 THEN
    WriteString("m2pkg: no entry module specified in manifest"); WriteLn;
    RAISE BuildError
  END;

  (* Ensure target/ directory exists *)
  Assign("target", outDir);
  rc := m2sys_mkdir_p(ADR(outDir));

  (* Construct output path *)
  GetName(name);
  Assign("target/", outPath);
  Concat(outPath, name, tmp);
  Assign(tmp, outPath);

  (* Start building command *)
  Assign("m2c", cmd);

  IF IsM2Plus() = 1 THEN
    Append(" --m2plus")
  END;

  (* Pass enabled features to compiler *)
  i := 0;
  WHILE i < FeatureCount() DO
    GetFeatureName(i, tok);
    IF IsFeatureEnabled(tok) = 1 THEN
      Append(" --feature ");
      Append(tok)
    END;
    INC(i)
  END;

  IF release = 1 THEN
    Append(" -O2")
  END;

  (* Add include paths from manifest *)
  GetIncludes(incl);
  IF Length(incl) > 0 THEN
    (* Split space-separated includes *)
    tpos := 0;
    tlen := Length(incl);
    WHILE tpos < tlen DO
      sp := tpos;
      WHILE (sp < tlen) AND (incl[sp] # ' ') DO INC(sp) END;
      IF sp > tpos THEN
        tok[0] := 0C;
        i := 0;
        WHILE tpos < sp DO
          tok[i] := incl[tpos];
          INC(i); INC(tpos)
        END;
        tok[i] := 0C;
        Append(" -I ");
        Append(tok)
      END;
      IF tpos < tlen THEN INC(tpos) END
    END
  END;

  (* Add -I for each dependency's src/ *)
  (* Use lockfile resolved paths if available, else manifest paths *)
  ndeps := DepCount();
  i := 0;
  WHILE i < ndeps DO
    IF IsDepLocal(i) = 1 THEN
      GetDepPath(i, depPath)
    ELSE
      (* Try lockfile resolved path *)
      depPath[0] := 0C;
      IF LockDepCount() > 0 THEN
        GetDepName(i, tok);
        rc := 0;
        WHILE rc < LockDepCount() DO
          GetDepLockName(rc, depSrc);
          IF CompareStr(tok, depSrc) = 0 THEN
            GetDepResolvedPath(rc, depPath);
            rc := LockDepCount() (* break *)
          END;
          INC(rc)
        END
      END;
      IF Length(depPath) = 0 THEN
        GetDepPath(i, depPath)
      END
    END;
    m2sys_join_path(ADR(depPath), ADR("src"), ADR(depSrc), 512);
    Append(" -I ");
    Append(depSrc);
    INC(i)
  END;

  (* Entry module *)
  Append(" ");
  Append(entry);

  (* Extra C files from package section *)
  nextra := ExtraCCount();
  i := 0;
  WHILE i < nextra DO
    GetExtraC(i, extraFile);
    Append(" ");
    Append(extraFile);
    INC(i)
  END;

  (* [cc] section: cflags via --cflag *)
  GetCCFlags(tok);
  IF Length(tok) > 0 THEN
    AppendCFlags(tok)
  END;

  (* [cc] section: libs *)
  i := 0;
  WHILE i < CCLibCount() DO
    GetCCLib(i, tok);
    Append(" ");
    Append(tok);
    INC(i)
  END;

  (* [cc] section: ldflags *)
  GetLDFlags(tok);
  IF Length(tok) > 0 THEN
    Append(" ");
    Append(tok)
  END;

  (* [cc] section: extra-c files *)
  i := 0;
  WHILE i < CCExtraCCount() DO
    GetCCExtraC(i, extraFile);
    Append(" ");
    Append(extraFile);
    INC(i)
  END;

  (* [cc] section: frameworks -> -framework <name> *)
  i := 0;
  WHILE i < CCFrameworkCount() DO
    GetCCFramework(i, tok);
    Append(" -framework ");
    Append(tok);
    INC(i)
  END;

  (* Transitive [cc] from dependencies *)
  AppendDepCC;

  (* Output *)
  Append(" -o ");
  Append(outPath);

  WriteString("m2pkg: "); WriteString(cmd); WriteLn;
  rc := m2sys_exec(ADR(cmd));
  IF rc # 0 THEN
    WriteString("m2pkg: build failed (exit "); WriteInt(rc, 1); WriteString(")"); WriteLn;
    RAISE BuildError
  END;
  WriteString("m2pkg: built "); WriteString(outPath); WriteLn
END Build;

PROCEDURE BuildAndRun(release: INTEGER; target: ARRAY OF CHAR);
VAR
  rc: INTEGER;
  name: ARRAY [0..63] OF CHAR;
  runCmd: ARRAY [0..511] OF CHAR;
BEGIN
  Build(release, target);

  GetName(name);
  Assign("./target/", runCmd);
  Concat(runCmd, name, tmp);
  Assign(tmp, runCmd);

  WriteString("m2pkg: running "); WriteString(runCmd); WriteLn;
  rc := m2sys_exec(ADR(runCmd));
  IF rc # 0 THEN
    RAISE BuildError
  END
END BuildAndRun;

END Builder.
