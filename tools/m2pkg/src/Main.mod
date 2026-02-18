MODULE Main;

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Strings IMPORT Assign, Length, CompareStr, Concat;
FROM Args IMPORT ArgCount, GetArg;
FROM SYSTEM IMPORT ADR;
FROM Sys IMPORT m2sys_file_exists, m2sys_exit, m2sys_is_dir, m2sys_rmdir_r;
FROM Manifest IMPORT Read, Clear, WriteTemplate, GetName, GetVersion,
                     IsM2Plus, GetEntry, DepCount, GetDepName, GetDepPath,
                     IsDepLocal, GetManifestVersion, GetEdition,
                     GetCCFlags, GetLDFlags, CCLibCount, GetCCLib,
                     CCFrameworkCount, GetCCFramework,
                     FeatureCount, GetFeatureName, GetFeatureDefault,
                     IsFeatureEnabled, SetRegistryURL;
FROM Builder IMPORT Build, BuildAndRun;
FROM Resolver IMPORT Resolve;
FROM Lockfile IMPORT Exists, WriteBoot, VerifyBoot;
FROM Registry IMPORT Publish;

VAR
  argc: CARDINAL;
  cmd: ARRAY [0..63] OF CHAR;
  arg: ARRAY [0..255] OF CHAR;
  release: INTEGER;
  target: ARRAY [0..63] OF CHAR;
  rc, i: INTEGER;
  name: ARRAY [0..63] OF CHAR;
  ver: ARRAY [0..31] OF CHAR;
  entry: ARRAY [0..255] OF CHAR;

PROCEDURE ShowVersion;
BEGIN
  WriteString("m2pkg 0.1.0"); WriteLn
END ShowVersion;

PROCEDURE ShowHelp;
BEGIN
  WriteString("m2pkg - Modula-2 package manager"); WriteLn;
  WriteString("Usage: m2pkg <command> [options]"); WriteLn;
  WriteLn;
  WriteString("Commands:"); WriteLn;
  WriteString("  init       Create a new m2.mod manifest"); WriteLn;
  WriteString("  build      Build the package"); WriteLn;
  WriteString("  run        Build and run the package"); WriteLn;
  WriteString("  resolve    Generate m2.lock from dependencies"); WriteLn;
  WriteString("  publish    Publish package to local registry"); WriteLn;
  WriteString("  fetch      Fetch registry dependencies to cache"); WriteLn;
  WriteString("  lock       Generate bootstrap.lock with hashes"); WriteLn;
  WriteString("  verify     Verify bootstrap.lock hashes"); WriteLn;
  WriteString("  check      Validate manifest and dependencies"); WriteLn;
  WriteString("  clean      Remove target/ directory"); WriteLn;
  WriteString("  version    Show version"); WriteLn;
  WriteLn;
  WriteString("Options:"); WriteLn;
  WriteString("  --release        Build with optimizations"); WriteLn;
  WriteString("  --target T       Set target triple"); WriteLn;
  WriteString("  --feature <name> Enable a feature"); WriteLn;
  WriteString("  --no-feature <n> Disable a feature"); WriteLn;
  WriteString("  --registry <url> Set registry URL"); WriteLn
END ShowHelp;

PROCEDURE DoInit;
VAR mf: ARRAY [0..15] OF CHAR;
BEGIN
  Assign("m2.mod", mf);
  IF m2sys_file_exists(ADR(mf)) = 1 THEN
    WriteString("m2pkg: m2.mod already exists"); WriteLn;
    RETURN
  END;
  rc := WriteTemplate("m2.mod");
  IF rc = 0 THEN
    WriteString("m2pkg: created m2.mod"); WriteLn
  ELSE
    WriteString("m2pkg: failed to create m2.mod"); WriteLn
  END
END DoInit;

PROCEDURE LoadManifest(): INTEGER;
BEGIN
  rc := Read("m2.mod");
  IF rc < 0 THEN
    WriteString("m2pkg: cannot read m2.mod (are you in a package directory?)"); WriteLn;
    RETURN 1
  END;
  RETURN 0
END LoadManifest;

PROCEDURE ParseBuildArgs;
BEGIN
  release := 0;
  target[0] := 0C;
  i := 2;
  WHILE i < VAL(INTEGER, argc) DO
    GetArg(i, arg);
    IF CompareStr(arg, "--release") = 0 THEN
      release := 1
    ELSIF CompareStr(arg, "--target") = 0 THEN
      INC(i);
      IF i < VAL(INTEGER, argc) THEN
        GetArg(i, target)
      END
    END;
    INC(i)
  END
END ParseBuildArgs;

PROCEDURE ParseRegistryArg;
(* Scan args for --registry <url> and set it on manifest *)
VAR j2: INTEGER;
    rarg: ARRAY [0..511] OF CHAR;
BEGIN
  j2 := 2;
  WHILE j2 < VAL(INTEGER, argc) DO
    GetArg(j2, rarg);
    IF CompareStr(rarg, "--registry") = 0 THEN
      INC(j2);
      IF j2 < VAL(INTEGER, argc) THEN
        GetArg(j2, rarg);
        SetRegistryURL(rarg)
      END
    END;
    INC(j2)
  END
END ParseRegistryArg;

PROCEDURE DoCheck;
VAR
  nd, j: INTEGER;
  depName: ARRAY [0..63] OF CHAR;
  depPath: ARRAY [0..255] OF CHAR;
  entryBuf: ARRAY [0..255] OF CHAR;
  nameBuf: ARRAY [0..63] OF CHAR;
  verBuf: ARRAY [0..31] OF CHAR;
  edBuf: ARRAY [0..31] OF CHAR;
  errors: INTEGER;
  lockPath: ARRAY [0..15] OF CHAR;
BEGIN
  errors := 0;

  (* Validate name *)
  GetName(nameBuf);
  IF Length(nameBuf) = 0 THEN
    WriteString("  error: 'name' is missing"); WriteLn;
    INC(errors)
  END;

  (* Validate version *)
  GetVersion(verBuf);
  IF Length(verBuf) = 0 THEN
    WriteString("  error: 'version' is missing"); WriteLn;
    INC(errors)
  END;

  (* Validate entry *)
  GetEntry(entryBuf);
  IF Length(entryBuf) = 0 THEN
    WriteString("  error: 'entry' is missing"); WriteLn;
    INC(errors)
  ELSIF m2sys_file_exists(ADR(entryBuf)) = 0 THEN
    WriteString("  error: entry file '");
    WriteString(entryBuf);
    WriteString("' not found"); WriteLn;
    INC(errors)
  END;

  (* Print edition *)
  GetEdition(edBuf);
  WriteString("  edition: "); WriteString(edBuf); WriteLn;

  (* Check manifest_version *)
  WriteString("  manifest_version: ");
  WriteInt(GetManifestVersion(), 1); WriteLn;

  (* Verify lockfile matches manifest if deps exist *)
  nd := DepCount();
  IF nd > 0 THEN
    Assign("m2.lock", lockPath);
    IF Exists("m2.lock") = 0 THEN
      WriteString("  warning: m2.lock not found but "); WriteInt(nd, 1);
      WriteString(" dependencies declared — run 'm2pkg resolve'"); WriteLn
    END
  END;

  (* Check all dep paths exist *)
  FOR j := 0 TO nd - 1 DO
    GetDepName(j, depName);
    GetDepPath(j, depPath);
    IF IsDepLocal(j) = 1 THEN
      IF m2sys_is_dir(ADR(depPath)) = 0 THEN
        WriteString("  error: dependency '"); WriteString(depName);
        WriteString("' path not found: "); WriteString(depPath); WriteLn;
        INC(errors)
      END
    END
  END;

  (* Report [cc] settings *)
  GetCCFlags(depPath);
  IF Length(depPath) > 0 THEN
    WriteString("  cc.cflags: "); WriteString(depPath); WriteLn
  END;
  GetLDFlags(depPath);
  IF Length(depPath) > 0 THEN
    WriteString("  cc.ldflags: "); WriteString(depPath); WriteLn
  END;
  IF CCLibCount() > 0 THEN
    WriteString("  cc.libs:");
    FOR j := 0 TO CCLibCount() - 1 DO
      GetCCLib(j, depName);
      WriteString(" "); WriteString(depName)
    END;
    WriteLn
  END;
  IF CCFrameworkCount() > 0 THEN
    WriteString("  cc.frameworks:");
    FOR j := 0 TO CCFrameworkCount() - 1 DO
      GetCCFramework(j, depName);
      WriteString(" "); WriteString(depName)
    END;
    WriteLn
  END;

  (* Report features *)
  IF FeatureCount() > 0 THEN
    WriteString("  features:");
    FOR j := 0 TO FeatureCount() - 1 DO
      GetFeatureName(j, depName);
      GetFeatureDefault(j, depPath);
      WriteString(" "); WriteString(depName);
      WriteString("="); WriteString(depPath)
    END;
    WriteLn
  END;

  IF errors = 0 THEN
    WriteString("m2pkg check: OK"); WriteLn
  ELSE
    WriteString("m2pkg check: "); WriteInt(errors, 1);
    WriteString(" error(s)"); WriteLn;
    m2sys_exit(1)
  END
END DoCheck;

PROCEDURE DoClean;
VAR
  targetDir: ARRAY [0..15] OF CHAR;
BEGIN
  Assign("target", targetDir);
  IF m2sys_is_dir(ADR(targetDir)) = 1 THEN
    rc := m2sys_rmdir_r(ADR(targetDir));
    IF rc = 0 THEN
      WriteString("m2pkg: cleaned target/"); WriteLn
    ELSE
      WriteString("m2pkg: failed to remove target/"); WriteLn;
      m2sys_exit(1)
    END
  ELSE
    WriteString("m2pkg: nothing to clean"); WriteLn
  END
END DoClean;

BEGIN
  argc := ArgCount();
  IF argc < 2 THEN
    ShowHelp;
    m2sys_exit(1)
  END;

  GetArg(1, cmd);

  IF CompareStr(cmd, "version") = 0 THEN
    ShowVersion

  ELSIF CompareStr(cmd, "init") = 0 THEN
    DoInit

  ELSIF CompareStr(cmd, "build") = 0 THEN
    IF LoadManifest() = 0 THEN
      ParseBuildArgs;
      rc := Build(release, target);
      IF rc # 0 THEN m2sys_exit(1) END
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "run") = 0 THEN
    IF LoadManifest() = 0 THEN
      ParseBuildArgs;
      rc := BuildAndRun(release, target);
      IF rc # 0 THEN m2sys_exit(1) END
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "resolve") = 0 THEN
    IF LoadManifest() = 0 THEN
      ParseRegistryArg;
      rc := Resolve();
      IF rc # 0 THEN m2sys_exit(1) END
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "check") = 0 THEN
    IF LoadManifest() = 0 THEN
      DoCheck
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "publish") = 0 THEN
    IF LoadManifest() = 0 THEN
      rc := Publish();
      IF rc # 0 THEN m2sys_exit(1) END
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "fetch") = 0 THEN
    IF LoadManifest() = 0 THEN
      ParseRegistryArg;
      rc := Resolve();
      IF rc # 0 THEN m2sys_exit(1) END
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "lock") = 0 THEN
    IF LoadManifest() = 0 THEN
      rc := WriteBoot("bootstrap.lock");
      IF rc = 0 THEN
        WriteString("m2pkg: wrote bootstrap.lock"); WriteLn
      ELSE
        WriteString("m2pkg: failed to write bootstrap.lock"); WriteLn;
        m2sys_exit(1)
      END
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "verify") = 0 THEN
    IF LoadManifest() = 0 THEN
      rc := VerifyBoot("bootstrap.lock");
      IF rc # 0 THEN m2sys_exit(1) END
    ELSE
      m2sys_exit(1)
    END

  ELSIF CompareStr(cmd, "clean") = 0 THEN
    DoClean

  ELSE
    WriteString("m2pkg: unknown command '"); WriteString(cmd);
    WriteString("'"); WriteLn;
    ShowHelp;
    m2sys_exit(1)
  END
END Main.
