IMPLEMENTATION MODULE Registry;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Length, Concat, CompareStr, Pos, Copy;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_home_dir, m2sys_join_path, m2sys_mkdir_p,
               m2sys_file_exists, m2sys_is_dir, m2sys_copy_file,
               m2sys_sha256_file, m2sys_tar_create_ex, m2sys_tar_extract,
               m2sys_fopen, m2sys_fclose, m2sys_fread_line, m2sys_fwrite_str,
               m2sys_flock, m2sys_funlock,
               m2sys_list_dir, m2sys_http_get;
FROM Manifest IMPORT GetName, GetVersion;
FROM Semver IMPORT Parse, Compare, MatchesRange, ToString, IsValid, Version;

VAR
  tmp1: ARRAY [0..1023] OF CHAR;
  tmp2: ARRAY [0..1023] OF CHAR;

PROCEDURE GetRegistryDir(VAR buf: ARRAY OF CHAR);
VAR home: ARRAY [0..511] OF CHAR;
    t: ARRAY [0..1023] OF CHAR;
BEGIN
  m2sys_home_dir(ADR(home), 512);
  m2sys_join_path(ADR(home), ADR(".m2pkg"), ADR(t), 1024);
  m2sys_join_path(ADR(t), ADR("registry"), ADR(buf), 1024)
END GetRegistryDir;

PROCEDURE GetCacheDir(VAR buf: ARRAY OF CHAR);
VAR home: ARRAY [0..511] OF CHAR;
    t: ARRAY [0..1023] OF CHAR;
BEGIN
  m2sys_home_dir(ADR(home), 512);
  m2sys_join_path(ADR(home), ADR(".m2pkg"), ADR(t), 1024);
  m2sys_join_path(ADR(t), ADR("cache"), ADR(buf), 1024)
END GetCacheDir;

PROCEDURE Init(): INTEGER;
VAR
  regDir: ARRAY [0..1023] OF CHAR;
  idxDir: ARRAY [0..1023] OF CHAR;
  pkgDir: ARRAY [0..1023] OF CHAR;
  cacheDir: ARRAY [0..1023] OF CHAR;
  rc: INTEGER;
BEGIN
  GetRegistryDir(regDir);
  rc := m2sys_mkdir_p(ADR(regDir));
  IF rc # 0 THEN RETURN -1 END;
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  rc := m2sys_mkdir_p(ADR(idxDir));
  IF rc # 0 THEN RETURN -1 END;
  m2sys_join_path(ADR(regDir), ADR("packages"), ADR(pkgDir), 1024);
  rc := m2sys_mkdir_p(ADR(pkgDir));
  IF rc # 0 THEN RETURN -1 END;
  GetCacheDir(cacheDir);
  rc := m2sys_mkdir_p(ADR(cacheDir));
  RETURN rc
END Init;

PROCEDURE WrLine(fh: INTEGER; s: ARRAY OF CHAR);
VAR nl: ARRAY [0..1] OF CHAR;
    rc: INTEGER;
BEGIN
  rc := m2sys_fwrite_str(fh, ADR(s));
  nl[0] := 12C; nl[1] := 0C;
  rc := m2sys_fwrite_str(fh, ADR(nl))
END WrLine;

PROCEDURE Publish(): INTEGER;
VAR
  name: ARRAY [0..63] OF CHAR;
  ver: ARRAY [0..31] OF CHAR;
  regDir: ARRAY [0..1023] OF CHAR;
  tarName: ARRAY [0..255] OF CHAR;
  tarPath: ARRAY [0..1023] OF CHAR;
  pkgDir: ARRAY [0..1023] OF CHAR;
  idxDir: ARRAY [0..1023] OF CHAR;
  idxPkgDir: ARRAY [0..1023] OF CHAR;
  idxFile: ARRAY [0..1023] OF CHAR;
  sha: ARRAY [0..64] OF CHAR;
  ln: ARRAY [0..511] OF CHAR;
  excl: ARRAY [0..15] OF CHAR;
  mfPath: ARRAY [0..15] OF CHAR;
  rc, fh, lockfh: INTEGER;
  wmode: ARRAY [0..1] OF CHAR;
  rmode: ARRAY [0..1] OF CHAR;
  mfLine: ARRAY [0..511] OF CHAR;
  latestVer: ARRAY [0..31] OF CHAR;
  pubVer, latVer: Version;
BEGIN
  GetName(name);
  GetVersion(ver);
  IF Length(name) = 0 THEN
    WriteString("m2pkg: no package name in manifest"); WriteLn;
    RETURN 1
  END;
  IF Length(ver) = 0 THEN
    WriteString("m2pkg: no version in manifest"); WriteLn;
    RETURN 1
  END;

  (* Semver enforcement *)
  IF IsValid(ver) = 0 THEN
    WriteString("m2pkg: version '"); WriteString(ver);
    WriteString("' is not valid semver (expected X.Y.Z)"); WriteLn;
    RETURN 1
  END;
  IF VersionExists(name, ver) = 1 THEN
    WriteString("m2pkg: version "); WriteString(ver);
    WriteString(" of "); WriteString(name);
    WriteString(" already published"); WriteLn;
    RETURN 1
  END;
  rc := GetLatestVersion(name, latestVer);
  IF (rc = 0) AND (Length(latestVer) > 0) THEN
    rc := Parse(ver, pubVer);
    IF rc = 0 THEN
      rc := Parse(latestVer, latVer);
      IF (rc = 0) AND (Compare(pubVer, latVer) < 0) THEN
        WriteString("m2pkg: version "); WriteString(ver);
        WriteString(" is less than latest published "); WriteString(latestVer); WriteLn;
        RETURN 1
      END
    END
  END;

  rc := Init();
  IF rc # 0 THEN
    WriteString("m2pkg: failed to initialize registry"); WriteLn;
    RETURN 1
  END;

  GetRegistryDir(regDir);
  m2sys_join_path(ADR(regDir), ADR("packages"), ADR(pkgDir), 1024);

  (* Build tarball name: name-version.tar *)
  Assign(name, tarName);
  Concat(tarName, "-", tmp1); Assign(tmp1, tarName);
  Concat(tarName, ver, tmp1); Assign(tmp1, tarName);
  Concat(tarName, ".tar", tmp1); Assign(tmp1, tarName);

  m2sys_join_path(ADR(pkgDir), ADR(tarName), ADR(tarPath), 1024);

  (* Create tarball excluding target/ *)
  Assign("target", excl);
  Assign(".", tmp1);
  rc := m2sys_tar_create_ex(ADR(tarPath), ADR(tmp1), ADR(excl));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create tarball"); WriteLn;
    RETURN 1
  END;

  (* Compute sha256 of tarball *)
  rc := m2sys_sha256_file(ADR(tarPath), ADR(sha));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to hash tarball"); WriteLn;
    RETURN 1
  END;

  (* Create index entry directory *)
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  m2sys_join_path(ADR(idxDir), ADR(name), ADR(idxPkgDir), 1024);
  rc := m2sys_mkdir_p(ADR(idxPkgDir));

  (* Write index file: copy of m2.mod + [registry] section *)
  Assign(ver, idxFile);
  Concat(idxFile, ".toml", tmp1); Assign(tmp1, idxFile);
  m2sys_join_path(ADR(idxPkgDir), ADR(idxFile), ADR(tmp2), 1024);
  Assign(tmp2, idxFile);

  (* Lock the index file for concurrency *)
  Assign("w", wmode);
  fh := m2sys_fopen(ADR(idxFile), ADR(wmode));
  IF fh < 0 THEN
    WriteString("m2pkg: failed to write index entry"); WriteLn;
    RETURN 1
  END;
  rc := m2sys_flock(fh, 1);

  (* Copy manifest content *)
  Assign("r", rmode);
  Assign("m2.mod", mfPath);
  lockfh := m2sys_fopen(ADR(mfPath), ADR(rmode));
  IF lockfh >= 0 THEN
    LOOP
      rc := m2sys_fread_line(lockfh, ADR(mfLine), 512);
      IF rc < 0 THEN EXIT END;
      WrLine(fh, mfLine)
    END;
    rc := m2sys_fclose(lockfh)
  END;

  (* Append [registry] section *)
  WrLine(fh, "");
  WrLine(fh, "[registry]");
  Assign("sha256=", ln);
  Concat(ln, sha, tmp1); Assign(tmp1, ln);
  WrLine(fh, ln);
  Assign("published=2026-02-18", ln);
  WrLine(fh, ln);

  rc := m2sys_funlock(fh);
  rc := m2sys_fclose(fh);

  WriteString("m2pkg: published "); WriteString(name);
  WriteString(" "); WriteString(ver);
  WriteString(" (sha256: "); WriteString(sha); WriteString(")"); WriteLn;
  RETURN 0
END Publish;

PROCEDURE Lookup(name: ARRAY OF CHAR; version: ARRAY OF CHAR;
                 VAR shaBuf: ARRAY OF CHAR): INTEGER;
VAR
  regDir: ARRAY [0..1023] OF CHAR;
  idxDir: ARRAY [0..1023] OF CHAR;
  idxPkgDir: ARRAY [0..1023] OF CHAR;
  idxFile: ARRAY [0..1023] OF CHAR;
  line: ARRAY [0..511] OF CHAR;
  fh, rc, n, eqPos, vlen: INTEGER;
  rmode: ARRAY [0..1] OF CHAR;
  key: ARRAY [0..63] OF CHAR;
  value: ARRAY [0..255] OF CHAR;
BEGIN
  shaBuf[0] := 0C;
  GetRegistryDir(regDir);
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  m2sys_join_path(ADR(idxDir), ADR(name), ADR(idxPkgDir), 1024);
  Assign(version, idxFile);
  Concat(idxFile, ".toml", tmp1); Assign(tmp1, idxFile);
  m2sys_join_path(ADR(idxPkgDir), ADR(idxFile), ADR(tmp2), 1024);
  Assign(tmp2, idxFile);

  IF m2sys_file_exists(ADR(idxFile)) = 0 THEN
    RETURN -1
  END;

  Assign("r", rmode);
  fh := m2sys_fopen(ADR(idxFile), ADR(rmode));
  IF fh < 0 THEN RETURN -1 END;

  LOOP
    n := m2sys_fread_line(fh, ADR(line), 512);
    IF n < 0 THEN EXIT END;
    IF Pos("sha256=", line) = 0 THEN
      vlen := Length(line) - 7;
      IF vlen > 0 THEN
        Copy(line, 7, vlen, shaBuf)
      END
    END
  END;
  rc := m2sys_fclose(fh);

  IF Length(shaBuf) > 0 THEN
    RETURN 0
  ELSE
    RETURN -1
  END
END Lookup;

PROCEDURE Fetch(name: ARRAY OF CHAR; version: ARRAY OF CHAR;
                VAR pathBuf: ARRAY OF CHAR): INTEGER;
VAR
  sha: ARRAY [0..64] OF CHAR;
  regDir: ARRAY [0..1023] OF CHAR;
  cacheDir: ARRAY [0..1023] OF CHAR;
  cachePkg: ARRAY [0..1023] OF CHAR;
  pkgDir: ARRAY [0..1023] OF CHAR;
  tarName: ARRAY [0..255] OF CHAR;
  tarPath: ARRAY [0..1023] OF CHAR;
  actualSha: ARRAY [0..64] OF CHAR;
  rc: INTEGER;
  dirName: ARRAY [0..127] OF CHAR;
BEGIN
  pathBuf[0] := 0C;

  (* Check if already cached *)
  GetCacheDir(cacheDir);
  Assign(name, dirName);
  Concat(dirName, "-", tmp1); Assign(tmp1, dirName);
  Concat(dirName, version, tmp1); Assign(tmp1, dirName);
  m2sys_join_path(ADR(cacheDir), ADR(dirName), ADR(cachePkg), 1024);

  IF m2sys_is_dir(ADR(cachePkg)) = 1 THEN
    Assign(cachePkg, pathBuf);
    RETURN 0
  END;

  (* Lookup in registry *)
  rc := Lookup(name, version, sha);
  IF rc # 0 THEN
    WriteString("m2pkg: package not found in registry: ");
    WriteString(name); WriteString(" "); WriteString(version); WriteLn;
    RETURN -1
  END;

  (* Find tarball *)
  GetRegistryDir(regDir);
  m2sys_join_path(ADR(regDir), ADR("packages"), ADR(pkgDir), 1024);
  Assign(name, tarName);
  Concat(tarName, "-", tmp1); Assign(tmp1, tarName);
  Concat(tarName, version, tmp1); Assign(tmp1, tarName);
  Concat(tarName, ".tar", tmp1); Assign(tmp1, tarName);
  m2sys_join_path(ADR(pkgDir), ADR(tarName), ADR(tarPath), 1024);

  IF m2sys_file_exists(ADR(tarPath)) = 0 THEN
    WriteString("m2pkg: tarball not found: "); WriteString(tarPath); WriteLn;
    RETURN -1
  END;

  (* Verify sha256 *)
  rc := m2sys_sha256_file(ADR(tarPath), ADR(actualSha));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to hash tarball"); WriteLn;
    RETURN -1
  END;
  IF CompareStr(sha, actualSha) # 0 THEN
    WriteString("m2pkg: sha256 mismatch for "); WriteString(name);
    WriteString(" "); WriteString(version); WriteLn;
    WriteString("  expected: "); WriteString(sha); WriteLn;
    WriteString("  actual:   "); WriteString(actualSha); WriteLn;
    RETURN -1
  END;

  (* Extract to cache *)
  rc := m2sys_mkdir_p(ADR(cachePkg));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create cache directory"); WriteLn;
    RETURN -1
  END;
  rc := m2sys_tar_extract(ADR(tarPath), ADR(cachePkg));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to extract tarball"); WriteLn;
    RETURN -1
  END;

  Assign(cachePkg, pathBuf);
  WriteString("m2pkg: fetched "); WriteString(name);
  WriteString(" "); WriteString(version);
  WriteString(" -> "); WriteString(cachePkg); WriteLn;
  RETURN 0
END Fetch;

PROCEDURE Verify(name: ARRAY OF CHAR; version: ARRAY OF CHAR): INTEGER;
VAR
  sha: ARRAY [0..64] OF CHAR;
  regDir: ARRAY [0..1023] OF CHAR;
  pkgDir: ARRAY [0..1023] OF CHAR;
  tarName: ARRAY [0..255] OF CHAR;
  tarPath: ARRAY [0..1023] OF CHAR;
  actualSha: ARRAY [0..64] OF CHAR;
  rc: INTEGER;
BEGIN
  rc := Lookup(name, version, sha);
  IF rc # 0 THEN RETURN -1 END;

  GetRegistryDir(regDir);
  m2sys_join_path(ADR(regDir), ADR("packages"), ADR(pkgDir), 1024);
  Assign(name, tarName);
  Concat(tarName, "-", tmp1); Assign(tmp1, tarName);
  Concat(tarName, version, tmp1); Assign(tmp1, tarName);
  Concat(tarName, ".tar", tmp1); Assign(tmp1, tarName);
  m2sys_join_path(ADR(pkgDir), ADR(tarName), ADR(tarPath), 1024);

  rc := m2sys_sha256_file(ADR(tarPath), ADR(actualSha));
  IF rc # 0 THEN RETURN -1 END;

  IF CompareStr(sha, actualSha) = 0 THEN
    RETURN 0
  ELSE
    RETURN -1
  END
END Verify;

PROCEDURE StripToml(s: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
(* Strip ".toml" suffix from filename to get version string *)
VAR slen, i: INTEGER;
BEGIN
  slen := Length(s);
  IF (slen > 5) AND (s[slen-5] = '.') AND (s[slen-4] = 't') AND
     (s[slen-3] = 'o') AND (s[slen-2] = 'm') AND (s[slen-1] = 'l') THEN
    i := 0;
    WHILE i < slen - 5 DO
      out[i] := s[i]; INC(i)
    END;
    out[i] := 0C
  ELSE
    Assign(s, out)
  END
END StripToml;

PROCEDURE LookupRange(name: ARRAY OF CHAR; rangeSpec: ARRAY OF CHAR;
                      VAR resolvedVersion: ARRAY OF CHAR;
                      VAR shaBuf: ARRAY OF CHAR): INTEGER;
VAR
  regDir: ARRAY [0..1023] OF CHAR;
  idxDir: ARRAY [0..1023] OF CHAR;
  idxPkgDir: ARRAY [0..1023] OF CHAR;
  dirBuf: ARRAY [0..4095] OF CHAR;
  entry: ARRAY [0..127] OF CHAR;
  verStr: ARRAY [0..63] OF CHAR;
  curVer, bestVer: Version;
  rc, bufLen, pos, epos, found: INTEGER;
BEGIN
  resolvedVersion[0] := 0C;
  shaBuf[0] := 0C;
  found := 0;

  GetRegistryDir(regDir);
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  m2sys_join_path(ADR(idxDir), ADR(name), ADR(idxPkgDir), 1024);

  IF m2sys_is_dir(ADR(idxPkgDir)) = 0 THEN RETURN -1 END;

  bufLen := m2sys_list_dir(ADR(idxPkgDir), ADR(dirBuf), 4096);
  IF bufLen <= 0 THEN RETURN -1 END;

  (* dirBuf is newline-separated list of filenames *)
  bestVer.major := -1; bestVer.minor := 0; bestVer.patch := 0;
  pos := 0;
  WHILE pos < bufLen DO
    epos := pos;
    WHILE (epos < bufLen) AND (dirBuf[epos] # 12C) AND (dirBuf[epos] # 0C) DO
      INC(epos)
    END;
    IF epos > pos THEN
      (* Extract entry name *)
      rc := 0;
      WHILE pos + rc < epos DO
        entry[rc] := dirBuf[pos + rc]; INC(rc)
      END;
      entry[rc] := 0C;

      (* Strip .toml suffix to get version string *)
      StripToml(entry, verStr);
      IF Parse(verStr, curVer) = 0 THEN
        IF MatchesRange(curVer, rangeSpec) = 1 THEN
          IF (found = 0) OR (Compare(curVer, bestVer) > 0) THEN
            bestVer := curVer;
            found := 1
          END
        END
      END
    END;
    pos := epos + 1
  END;

  IF found = 0 THEN RETURN -1 END;

  ToString(bestVer, resolvedVersion);
  rc := Lookup(name, resolvedVersion, shaBuf);
  RETURN rc
END LookupRange;

PROCEDURE VersionExists(name: ARRAY OF CHAR; version: ARRAY OF CHAR): INTEGER;
VAR
  regDir: ARRAY [0..1023] OF CHAR;
  idxDir: ARRAY [0..1023] OF CHAR;
  idxPkgDir: ARRAY [0..1023] OF CHAR;
  idxFile: ARRAY [0..1023] OF CHAR;
BEGIN
  GetRegistryDir(regDir);
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  m2sys_join_path(ADR(idxDir), ADR(name), ADR(idxPkgDir), 1024);
  Assign(version, idxFile);
  Concat(idxFile, ".toml", tmp1); Assign(tmp1, idxFile);
  m2sys_join_path(ADR(idxPkgDir), ADR(idxFile), ADR(tmp2), 1024);
  Assign(tmp2, idxFile);
  RETURN m2sys_file_exists(ADR(idxFile))
END VersionExists;

PROCEDURE GetLatestVersion(name: ARRAY OF CHAR; VAR latestBuf: ARRAY OF CHAR): INTEGER;
VAR
  regDir: ARRAY [0..1023] OF CHAR;
  idxDir: ARRAY [0..1023] OF CHAR;
  idxPkgDir: ARRAY [0..1023] OF CHAR;
  dirBuf: ARRAY [0..4095] OF CHAR;
  entry: ARRAY [0..127] OF CHAR;
  verStr: ARRAY [0..63] OF CHAR;
  curVer, bestVer: Version;
  rc, bufLen, pos, epos, found: INTEGER;
BEGIN
  latestBuf[0] := 0C;
  found := 0;

  GetRegistryDir(regDir);
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  m2sys_join_path(ADR(idxDir), ADR(name), ADR(idxPkgDir), 1024);

  IF m2sys_is_dir(ADR(idxPkgDir)) = 0 THEN RETURN -1 END;

  bufLen := m2sys_list_dir(ADR(idxPkgDir), ADR(dirBuf), 4096);
  IF bufLen <= 0 THEN RETURN -1 END;

  bestVer.major := -1; bestVer.minor := 0; bestVer.patch := 0;
  pos := 0;
  WHILE pos < bufLen DO
    epos := pos;
    WHILE (epos < bufLen) AND (dirBuf[epos] # 12C) AND (dirBuf[epos] # 0C) DO
      INC(epos)
    END;
    IF epos > pos THEN
      rc := 0;
      WHILE pos + rc < epos DO
        entry[rc] := dirBuf[pos + rc]; INC(rc)
      END;
      entry[rc] := 0C;
      StripToml(entry, verStr);
      IF Parse(verStr, curVer) = 0 THEN
        IF (found = 0) OR (Compare(curVer, bestVer) > 0) THEN
          bestVer := curVer;
          found := 1
        END
      END
    END;
    pos := epos + 1
  END;

  IF found = 0 THEN RETURN -1 END;
  ToString(bestVer, latestBuf);
  RETURN 0
END GetLatestVersion;

PROCEDURE FetchRemote(url: ARRAY OF CHAR; name: ARRAY OF CHAR;
                      version: ARRAY OF CHAR; VAR pathBuf: ARRAY OF CHAR): INTEGER;
VAR
  cacheDir: ARRAY [0..1023] OF CHAR;
  cachePkg: ARRAY [0..1023] OF CHAR;
  dirName: ARRAY [0..127] OF CHAR;
  idxUrl: ARRAY [0..1023] OF CHAR;
  tarUrl: ARRAY [0..1023] OF CHAR;
  idxPath: ARRAY [0..1023] OF CHAR;
  tarPath: ARRAY [0..1023] OF CHAR;
  sha: ARRAY [0..64] OF CHAR;
  actualSha: ARRAY [0..64] OF CHAR;
  line: ARRAY [0..511] OF CHAR;
  fh, rc, n, vlen: INTEGER;
  rmode: ARRAY [0..1] OF CHAR;
BEGIN
  pathBuf[0] := 0C;

  (* Check if already cached *)
  GetCacheDir(cacheDir);
  Assign(name, dirName);
  Concat(dirName, "-", tmp1); Assign(tmp1, dirName);
  Concat(dirName, version, tmp1); Assign(tmp1, dirName);
  m2sys_join_path(ADR(cacheDir), ADR(dirName), ADR(cachePkg), 1024);
  IF m2sys_is_dir(ADR(cachePkg)) = 1 THEN
    Assign(cachePkg, pathBuf);
    RETURN 0
  END;

  (* Build index URL: <url>/index/<name>/<version>.toml *)
  Assign(url, idxUrl);
  Concat(idxUrl, "/index/", tmp1); Assign(tmp1, idxUrl);
  Concat(idxUrl, name, tmp1); Assign(tmp1, idxUrl);
  Concat(idxUrl, "/", tmp1); Assign(tmp1, idxUrl);
  Concat(idxUrl, version, tmp1); Assign(tmp1, idxUrl);
  Concat(idxUrl, ".toml", tmp1); Assign(tmp1, idxUrl);

  (* Download index entry to temp file *)
  m2sys_join_path(ADR(cacheDir), ADR("_remote_idx.toml"), ADR(idxPath), 1024);
  rc := m2sys_http_get(ADR(idxUrl), ADR(idxPath));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to fetch index from "); WriteString(idxUrl); WriteLn;
    RETURN -1
  END;

  (* Read sha256 from downloaded index entry *)
  sha[0] := 0C;
  Assign("r", rmode);
  fh := m2sys_fopen(ADR(idxPath), ADR(rmode));
  IF fh >= 0 THEN
    LOOP
      n := m2sys_fread_line(fh, ADR(line), 512);
      IF n < 0 THEN EXIT END;
      IF Pos("sha256=", line) = 0 THEN
        vlen := Length(line) - 7;
        IF vlen > 0 THEN Copy(line, 7, vlen, sha) END
      END
    END;
    rc := m2sys_fclose(fh)
  END;
  IF Length(sha) = 0 THEN
    WriteString("m2pkg: no sha256 in remote index for "); WriteString(name); WriteLn;
    RETURN -1
  END;

  (* Build tarball URL: <url>/packages/<name>-<version>.tar *)
  Assign(url, tarUrl);
  Concat(tarUrl, "/packages/", tmp1); Assign(tmp1, tarUrl);
  Concat(tarUrl, name, tmp1); Assign(tmp1, tarUrl);
  Concat(tarUrl, "-", tmp1); Assign(tmp1, tarUrl);
  Concat(tarUrl, version, tmp1); Assign(tmp1, tarUrl);
  Concat(tarUrl, ".tar", tmp1); Assign(tmp1, tarUrl);

  (* Download tarball to cache *)
  m2sys_join_path(ADR(cacheDir), ADR(dirName), ADR(tarPath), 1024);
  Concat(tarPath, ".tar", tmp1); Assign(tmp1, tarPath);
  rc := m2sys_http_get(ADR(tarUrl), ADR(tarPath));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to fetch tarball from "); WriteString(tarUrl); WriteLn;
    RETURN -1
  END;

  (* Verify sha256 *)
  rc := m2sys_sha256_file(ADR(tarPath), ADR(actualSha));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to hash remote tarball"); WriteLn;
    RETURN -1
  END;
  IF CompareStr(sha, actualSha) # 0 THEN
    WriteString("m2pkg: sha256 mismatch for remote "); WriteString(name);
    WriteString(" "); WriteString(version); WriteLn;
    RETURN -1
  END;

  (* Extract to cache *)
  rc := m2sys_mkdir_p(ADR(cachePkg));
  IF rc # 0 THEN RETURN -1 END;
  rc := m2sys_tar_extract(ADR(tarPath), ADR(cachePkg));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to extract remote tarball"); WriteLn;
    RETURN -1
  END;

  Assign(cachePkg, pathBuf);
  WriteString("m2pkg: fetched remote "); WriteString(name);
  WriteString(" "); WriteString(version);
  WriteString(" -> "); WriteString(cachePkg); WriteLn;
  RETURN 0
END FetchRemote;

END Registry.
