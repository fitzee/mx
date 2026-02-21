IMPLEMENTATION MODULE Builder;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Concat, Length, CompareStr;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Sys IMPORT m2sys_exec, m2sys_file_exists, m2sys_getcwd,
               m2sys_join_path, m2sys_mkdir_p;
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

(* Append transitive [cc] flags from all local dependencies *)
PROCEDURE AppendDepCC;
VAR
  i, j, ndeps2, nlibs, nfws, nexcc: INTEGER;
  depPath2: ARRAY [0..255] OF CHAR;
  depManifest: ARRAY [0..511] OF CHAR;
  ccBuf: ARRAY [0..1023] OF CHAR;
  libBuf: ARRAY [0..63] OF CHAR;
  fwBuf: ARRAY [0..63] OF CHAR;
  ecBuf: ARRAY [0..255] OF CHAR;
BEGIN
  ndeps2 := DepCount();
  j := 0;
  WHILE j < ndeps2 DO
    IF IsDepLocal(j) = 1 THEN
      GetDepPath(j, depPath2);
      (* Build path to dep's m2.toml *)
      Assign(depPath2, depManifest);
      Concat(depManifest, "/m2.toml", tmp);
      Assign(tmp, depManifest);
      (* Save current manifest state, read dep manifest *)
      Clear;
      Read(depManifest);
      (* Append dep's cflags *)
      GetCCFlags(ccBuf);
      IF Length(ccBuf) > 0 THEN
        Append(" ");
        Append(ccBuf)
      END;
      (* Append dep's libs *)
      nlibs := CCLibCount();
      i := 0;
      WHILE i < nlibs DO
        GetCCLib(i, libBuf);
        Append(" -l");
        Append(libBuf);
        INC(i)
      END;
      (* Append dep's ldflags *)
      GetLDFlags(ccBuf);
      IF Length(ccBuf) > 0 THEN
        Append(" ");
        Append(ccBuf)
      END;
      (* Append dep's extra-c with dep path prefix *)
      nexcc := CCExtraCCount();
      i := 0;
      WHILE i < nexcc DO
        GetCCExtraC(i, ecBuf);
        Append(" ");
        Append(depPath2);
        Append("/");
        Append(ecBuf);
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

  (* [cc] section: cflags *)
  GetCCFlags(tok);
  IF Length(tok) > 0 THEN
    Append(" ");
    Append(tok)
  END;

  (* [cc] section: libs -> -l<name> *)
  i := 0;
  WHILE i < CCLibCount() DO
    GetCCLib(i, tok);
    Append(" -l");
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
