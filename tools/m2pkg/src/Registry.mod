IMPLEMENTATION MODULE Registry;

FROM SYSTEM IMPORT ADR, ADDRESS;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Strings IMPORT Assign, Length, Concat, CompareStr, Pos, Copy;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Sys IMPORT m2sys_home_dir, m2sys_join_path, m2sys_mkdir_p,
               m2sys_file_exists, m2sys_is_dir, m2sys_copy_file,
               m2sys_sha256_file, m2sys_tar_create_ex, m2sys_tar_extract,
               m2sys_fopen, m2sys_fclose, m2sys_fread_line, m2sys_fwrite_str,
               m2sys_flock, m2sys_funlock,
               m2sys_list_dir, m2sys_getenv, m2sys_remove_file,
               m2sys_file_size, m2sys_fread_bytes;
FROM Manifest IMPORT GetName, GetVersion;
FROM Semver IMPORT Parse, Compare, MatchesRange, ToString, IsValid, Version;
FROM SyncHTTP IMPORT SyncGet, SyncDownload, SyncPut;
FROM HTTPClient IMPORT ResponsePtr, FreeResponse;
IMPORT HTTPClient;
IMPORT Buffers;
IMPORT Auth;

VAR
  tmp1: ARRAY [0..1023] OF CHAR;
  tmp2: ARRAY [0..1023] OF CHAR;
  mInsecure: BOOLEAN;

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

PROCEDURE Init;
VAR
  regDir: ARRAY [0..1023] OF CHAR;
  idxDir: ARRAY [0..1023] OF CHAR;
  pkgDir: ARRAY [0..1023] OF CHAR;
  cacheDir: ARRAY [0..1023] OF CHAR;
  rc: INTEGER;
BEGIN
  GetRegistryDir(regDir);
  rc := m2sys_mkdir_p(ADR(regDir));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create registry directory"); WriteLn;
    RAISE RegistryError
  END;
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  rc := m2sys_mkdir_p(ADR(idxDir));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create registry index directory"); WriteLn;
    RAISE RegistryError
  END;
  m2sys_join_path(ADR(regDir), ADR("packages"), ADR(pkgDir), 1024);
  rc := m2sys_mkdir_p(ADR(pkgDir));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create registry packages directory"); WriteLn;
    RAISE RegistryError
  END;
  GetCacheDir(cacheDir);
  rc := m2sys_mkdir_p(ADR(cacheDir));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create cache directory"); WriteLn;
    RAISE RegistryError
  END
END Init;

PROCEDURE WrLine(fh: INTEGER; s: ARRAY OF CHAR);
VAR nl: ARRAY [0..1] OF CHAR;
    rc: INTEGER;
BEGIN
  rc := m2sys_fwrite_str(fh, ADR(s));
  nl[0] := 12C; nl[1] := 0C;
  rc := m2sys_fwrite_str(fh, ADR(nl))
END WrLine;

PROCEDURE Publish;
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
    RAISE RegistryError
  END;
  IF Length(ver) = 0 THEN
    WriteString("m2pkg: no version in manifest"); WriteLn;
    RAISE RegistryError
  END;

  (* Semver enforcement *)
  IF IsValid(ver) = 0 THEN
    WriteString("m2pkg: version '"); WriteString(ver);
    WriteString("' is not valid semver (expected X.Y.Z)"); WriteLn;
    RAISE RegistryError
  END;
  IF VersionExists(name, ver) = 1 THEN
    WriteString("m2pkg: version "); WriteString(ver);
    WriteString(" of "); WriteString(name);
    WriteString(" already published"); WriteLn;
    RAISE RegistryError
  END;
  rc := GetLatestVersion(name, latestVer);
  IF (rc = 0) AND (Length(latestVer) > 0) THEN
    rc := Parse(ver, pubVer);
    IF rc = 0 THEN
      rc := Parse(latestVer, latVer);
      IF (rc = 0) AND (Compare(pubVer, latVer) < 0) THEN
        WriteString("m2pkg: version "); WriteString(ver);
        WriteString(" is less than latest published "); WriteString(latestVer); WriteLn;
        RAISE RegistryError
      END
    END
  END;

  Init;

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
    RAISE RegistryError
  END;

  (* Compute sha256 of tarball *)
  rc := m2sys_sha256_file(ADR(tarPath), ADR(sha));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to hash tarball"); WriteLn;
    RAISE RegistryError
  END;

  (* Create index entry directory *)
  m2sys_join_path(ADR(regDir), ADR("index"), ADR(idxDir), 1024);
  m2sys_join_path(ADR(idxDir), ADR(name), ADR(idxPkgDir), 1024);
  rc := m2sys_mkdir_p(ADR(idxPkgDir));

  (* Write index file: copy of m2.toml + [registry] section *)
  Assign(ver, idxFile);
  Concat(idxFile, ".toml", tmp1); Assign(tmp1, idxFile);
  m2sys_join_path(ADR(idxPkgDir), ADR(idxFile), ADR(tmp2), 1024);
  Assign(tmp2, idxFile);

  (* Lock the index file for concurrency *)
  Assign("w", wmode);
  fh := m2sys_fopen(ADR(idxFile), ADR(wmode));
  IF fh < 0 THEN
    WriteString("m2pkg: failed to write index entry"); WriteLn;
    RAISE RegistryError
  END;
  rc := m2sys_flock(fh, 1);
  TRY
    (* Copy manifest content *)
    Assign("r", rmode);
    Assign("m2.toml", mfPath);
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
    WrLine(fh, ln)
  FINALLY
    rc := m2sys_funlock(fh);
    rc := m2sys_fclose(fh)
  END;

  WriteString("m2pkg: published "); WriteString(name);
  WriteString(" "); WriteString(ver);
  WriteString(" (sha256: "); WriteString(sha); WriteString(")"); WriteLn
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
                VAR pathBuf: ARRAY OF CHAR);
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
    RETURN
  END;

  (* Lookup in registry *)
  rc := Lookup(name, version, sha);
  IF rc # 0 THEN
    WriteString("m2pkg: package not found in registry: ");
    WriteString(name); WriteString(" "); WriteString(version); WriteLn;
    RAISE RegistryError
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
    RAISE RegistryError
  END;

  (* Verify sha256 *)
  rc := m2sys_sha256_file(ADR(tarPath), ADR(actualSha));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to hash tarball"); WriteLn;
    RAISE RegistryError
  END;
  IF CompareStr(sha, actualSha) # 0 THEN
    WriteString("m2pkg: sha256 mismatch for "); WriteString(name);
    WriteString(" "); WriteString(version); WriteLn;
    WriteString("  expected: "); WriteString(sha); WriteLn;
    WriteString("  actual:   "); WriteString(actualSha); WriteLn;
    RAISE RegistryError
  END;

  (* Extract to cache *)
  rc := m2sys_mkdir_p(ADR(cachePkg));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create cache directory"); WriteLn;
    RAISE RegistryError
  END;
  rc := m2sys_tar_extract(ADR(tarPath), ADR(cachePkg));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to extract tarball"); WriteLn;
    RAISE RegistryError
  END;

  Assign(cachePkg, pathBuf);
  WriteString("m2pkg: fetched "); WriteString(name);
  WriteString(" "); WriteString(version);
  WriteString(" -> "); WriteString(cachePkg); WriteLn
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
                      version: ARRAY OF CHAR; VAR pathBuf: ARRAY OF CHAR);
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
  fh, rc, n, vlen, httpStatus: INTEGER;
  rmode: ARRAY [0..1] OF CHAR;
  st: HTTPClient.Status;
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
    RETURN
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
  st := SyncDownload(idxUrl, mInsecure, idxPath, httpStatus);
  IF st # HTTPClient.OK THEN
    WriteString("m2pkg: failed to connect to "); WriteString(url); WriteLn;
    RAISE RegistryError
  END;
  IF httpStatus # 200 THEN
    HttpErrorMsg(httpStatus, name, version);
    RAISE RegistryError
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
    RAISE RegistryError
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
  st := SyncDownload(tarUrl, mInsecure, tarPath, httpStatus);
  IF st # HTTPClient.OK THEN
    WriteString("m2pkg: failed to connect to "); WriteString(url); WriteLn;
    RAISE RegistryError
  END;
  IF httpStatus # 200 THEN
    HttpErrorMsg(httpStatus, name, version);
    RAISE RegistryError
  END;

  (* Verify sha256 *)
  rc := m2sys_sha256_file(ADR(tarPath), ADR(actualSha));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to hash remote tarball"); WriteLn;
    RAISE RegistryError
  END;
  IF CompareStr(sha, actualSha) # 0 THEN
    WriteString("m2pkg: sha256 mismatch for remote "); WriteString(name);
    WriteString(" "); WriteString(version); WriteLn;
    RAISE RegistryError
  END;

  (* Extract to cache *)
  rc := m2sys_mkdir_p(ADR(cachePkg));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create cache directory"); WriteLn;
    RAISE RegistryError
  END;
  rc := m2sys_tar_extract(ADR(tarPath), ADR(cachePkg));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to extract remote tarball"); WriteLn;
    RAISE RegistryError
  END;

  Assign(cachePkg, pathBuf);
  WriteString("m2pkg: fetched remote "); WriteString(name);
  WriteString(" "); WriteString(version);
  WriteString(" -> "); WriteString(cachePkg); WriteLn
END FetchRemote;

(* Extract a JSON string value for a given key.
   Scans for "key":"value" patterns. Returns length of value, 0 if not found. *)
PROCEDURE ExtractJsonValue(VAR json: ARRAY OF CHAR;
                           VAR key: ARRAY OF CHAR;
                           VAR out: ARRAY OF CHAR): INTEGER;
VAR
  jlen, klen, i, j, oi, matchStart: INTEGER;
  inQuote: BOOLEAN;
BEGIN
  out[0] := 0C;
  jlen := Length(json);
  klen := Length(key);
  i := 0;

  WHILE i < jlen - klen - 3 DO
    (* Look for "key" *)
    IF json[i] = '"' THEN
      j := 0;
      matchStart := i + 1;
      WHILE (j < klen) AND (matchStart + j < jlen) AND
            (json[matchStart + j] = key[j]) DO
        INC(j)
      END;
      IF (j = klen) AND (matchStart + j < jlen) AND
         (json[matchStart + j] = '"') THEN
        (* Found "key" — skip to colon and opening quote *)
        i := matchStart + j + 1;
        WHILE (i < jlen) AND ((json[i] = ':') OR (json[i] = ' ')) DO
          INC(i)
        END;
        IF (i < jlen) AND (json[i] = '"') THEN
          INC(i);
          oi := 0;
          WHILE (i < jlen) AND (json[i] # '"') AND (oi <= HIGH(out) - 1) DO
            out[oi] := json[i];
            INC(oi); INC(i)
          END;
          out[oi] := 0C;
          RETURN oi
        END
      END
    END;
    INC(i)
  END;
  RETURN 0
END ExtractJsonValue;

PROCEDURE FetchLatest(url: ARRAY OF CHAR; name: ARRAY OF CHAR;
                      VAR verBuf: ARRAY OF CHAR;
                      VAR shaBuf: ARRAY OF CHAR): INTEGER;
VAR
  apiUrl: ARRAY [0..1023] OF CHAR;
  resp: ResponsePtr;
  st: HTTPClient.Status;
  body: ARRAY [0..4095] OF CHAR;
  bodyLen, vl: INTEGER;
  bst: Buffers.Status;
  vkey: ARRAY [0..7] OF CHAR;
  skey: ARRAY [0..6] OF CHAR;
BEGIN
  verBuf[0] := 0C;
  shaBuf[0] := 0C;

  (* Build API URL: <url>/api/v1/packages/<name>/latest *)
  Assign(url, apiUrl);
  Concat(apiUrl, "/api/v1/packages/", tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, name, tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, "/latest", tmp1); Assign(tmp1, apiUrl);

  st := SyncGet(apiUrl, mInsecure, resp);
  IF st # HTTPClient.OK THEN RETURN -1 END;
  IF resp = NIL THEN RETURN -1 END;

  IF resp^.statusCode # 200 THEN
    HttpErrorMsg(resp^.statusCode, name, "");
    FreeResponse(resp);
    RETURN -1
  END;

  (* Extract body text *)
  bodyLen := Buffers.Length(resp^.body);
  IF bodyLen > 4095 THEN bodyLen := 4095 END;
  IF bodyLen > 0 THEN
    bst := Buffers.CopyOut(resp^.body, 0, bodyLen, body)
  END;
  body[bodyLen] := 0C;
  FreeResponse(resp);

  (* Parse JSON: {"version":"X.Y.Z","sha256":"abc..."} *)
  Assign("version", vkey);
  vl := ExtractJsonValue(body, vkey, verBuf);
  IF vl = 0 THEN RETURN -1 END;

  Assign("sha256", skey);
  vl := ExtractJsonValue(body, skey, shaBuf);

  RETURN 0
END FetchLatest;

PROCEDURE HttpErrorMsg(httpStatus: INTEGER; name: ARRAY OF CHAR;
                       version: ARRAY OF CHAR);
BEGIN
  IF httpStatus = 404 THEN
    WriteString("m2pkg: package not found on remote registry: ");
    WriteString(name); WriteString(" "); WriteString(version); WriteLn
  ELSIF httpStatus = 401 THEN
    WriteString("m2pkg: authentication required (set M2PKG_TOKEN)"); WriteLn
  ELSIF httpStatus = 409 THEN
    WriteString("m2pkg: version already exists: ");
    WriteString(name); WriteString(" "); WriteString(version); WriteLn
  ELSIF httpStatus = 413 THEN
    WriteString("m2pkg: package exceeds max size"); WriteLn
  ELSIF httpStatus >= 500 THEN
    WriteString("m2pkg: remote registry error (HTTP ");
    WriteInt(httpStatus, 1); WriteString(")"); WriteLn
  ELSE
    WriteString("m2pkg: remote registry HTTP error ");
    WriteInt(httpStatus, 1); WriteLn
  END
END HttpErrorMsg;

(* Extract a JSON array of strings for a given key.
   Writes newline-separated values into out. Returns count. *)
PROCEDURE ExtractJsonArray(VAR json: ARRAY OF CHAR;
                           VAR key: ARRAY OF CHAR;
                           VAR out: ARRAY OF CHAR): INTEGER;
VAR
  jlen, klen, i, j, oi, matchStart, count: INTEGER;
BEGIN
  out[0] := 0C;
  jlen := Length(json);
  klen := Length(key);
  i := 0;
  count := 0;
  oi := 0;

  (* Find "key" *)
  WHILE i < jlen - klen - 3 DO
    IF json[i] = '"' THEN
      j := 0;
      matchStart := i + 1;
      WHILE (j < klen) AND (matchStart + j < jlen) AND
            (json[matchStart + j] = key[j]) DO
        INC(j)
      END;
      IF (j = klen) AND (matchStart + j < jlen) AND
         (json[matchStart + j] = '"') THEN
        (* Found "key" — skip to [ *)
        i := matchStart + j + 1;
        WHILE (i < jlen) AND (json[i] # '[') DO INC(i) END;
        IF i >= jlen THEN RETURN 0 END;
        INC(i); (* skip [ *)

        (* Parse array of strings *)
        WHILE (i < jlen) AND (json[i] # ']') DO
          (* Skip whitespace and commas *)
          WHILE (i < jlen) AND ((json[i] = ' ') OR (json[i] = ',') OR
                (json[i] = 12C) OR (json[i] = 11C)) DO
            INC(i)
          END;
          IF (i < jlen) AND (json[i] = '"') THEN
            INC(i);
            IF (count > 0) AND (oi <= HIGH(out) - 1) THEN
              out[oi] := 12C; INC(oi)
            END;
            WHILE (i < jlen) AND (json[i] # '"') AND (oi <= HIGH(out) - 1) DO
              out[oi] := json[i];
              INC(oi); INC(i)
            END;
            IF (i < jlen) AND (json[i] = '"') THEN INC(i) END;
            INC(count)
          ELSE
            INC(i)
          END
        END;
        out[oi] := 0C;
        RETURN count
      END
    END;
    INC(i)
  END;
  RETURN 0
END ExtractJsonArray;

(* Parse JSON results array: [{"name":"...","latest":"..."},...] *)
PROCEDURE ExtractSearchResults(VAR json: ARRAY OF CHAR);
VAR
  jlen, i, oi: INTEGER;
  nameStr: ARRAY [0..127] OF CHAR;
  latStr: ARRAY [0..31] OF CHAR;
  inResults: BOOLEAN;
BEGIN
  jlen := Length(json);
  i := 0;
  inResults := FALSE;

  (* Find "results" key then [ *)
  WHILE i < jlen - 8 DO
    IF (json[i] = '"') AND (json[i+1] = 'r') AND (json[i+2] = 'e') AND
       (json[i+3] = 's') AND (json[i+4] = 'u') AND (json[i+5] = 'l') AND
       (json[i+6] = 't') AND (json[i+7] = 's') AND (json[i+8] = '"') THEN
      i := i + 9;
      WHILE (i < jlen) AND (json[i] # '[') DO INC(i) END;
      IF i < jlen THEN inResults := TRUE; INC(i) END;
      (* i now points past [ in results array *)
      WHILE (i < jlen) AND (json[i] # ']') DO LOOP
        (* Find start of object *)
        WHILE (i < jlen) AND (json[i] # '{') AND (json[i] # ']') DO INC(i) END;
        IF (i >= jlen) OR (json[i] = ']') THEN EXIT END;
        INC(i); (* skip { *)

        (* Extract fields from this object *)
        nameStr[0] := 0C; latStr[0] := 0C;
        WHILE (i < jlen) AND (json[i] # '}') DO
          (* Skip to next key *)
          WHILE (i < jlen) AND (json[i] # '"') AND (json[i] # '}') DO INC(i) END;
          IF (i >= jlen) OR (json[i] = '}') THEN EXIT END;
          INC(i); (* skip opening " of key *)

          IF (i + 4 < jlen) AND (json[i] = 'n') AND (json[i+1] = 'a') AND
             (json[i+2] = 'm') AND (json[i+3] = 'e') AND (json[i+4] = '"') THEN
            i := i + 5;
            WHILE (i < jlen) AND ((json[i] = ':') OR (json[i] = ' ')) DO INC(i) END;
            IF (i < jlen) AND (json[i] = '"') THEN
              INC(i); oi := 0;
              WHILE (i < jlen) AND (json[i] # '"') AND (oi < 127) DO
                nameStr[oi] := json[i]; INC(oi); INC(i)
              END;
              nameStr[oi] := 0C;
              IF (i < jlen) AND (json[i] = '"') THEN INC(i) END
            END
          ELSIF (i + 6 < jlen) AND (json[i] = 'l') AND (json[i+1] = 'a') AND
                (json[i+2] = 't') AND (json[i+3] = 'e') AND (json[i+4] = 's') AND
                (json[i+5] = 't') AND (json[i+6] = '"') THEN
            i := i + 7;
            WHILE (i < jlen) AND ((json[i] = ':') OR (json[i] = ' ')) DO INC(i) END;
            IF (i < jlen) AND (json[i] = '"') THEN
              INC(i); oi := 0;
              WHILE (i < jlen) AND (json[i] # '"') AND (oi < 31) DO
                latStr[oi] := json[i]; INC(oi); INC(i)
              END;
              latStr[oi] := 0C;
              IF (i < jlen) AND (json[i] = '"') THEN INC(i) END
            END
          ELSE
            (* Skip unknown key *)
            WHILE (i < jlen) AND (json[i] # '"') DO INC(i) END;
            IF (i < jlen) THEN INC(i) END;
            (* Skip : and value *)
            WHILE (i < jlen) AND (json[i] # ',') AND (json[i] # '}') DO INC(i) END
          END
        END;
        IF (i < jlen) AND (json[i] = '}') THEN INC(i) END;

        (* Print result *)
        IF Length(nameStr) > 0 THEN
          WriteString("  "); WriteString(nameStr);
          IF Length(latStr) > 0 THEN
            WriteString(" (latest: "); WriteString(latStr); WriteString(")")
          END;
          WriteLn
        END;
        EXIT
      END END; (* WHILE/LOOP *)
      RETURN
    END;
    INC(i)
  END
END ExtractSearchResults;

PROCEDURE PublishRemote(url: ARRAY OF CHAR; token: ARRAY OF CHAR);
VAR
  name: ARRAY [0..63] OF CHAR;
  ver: ARRAY [0..31] OF CHAR;
  cacheDir: ARRAY [0..1023] OF CHAR;
  tarPath: ARRAY [0..1023] OF CHAR;
  tarName: ARRAY [0..255] OF CHAR;
  apiUrl: ARRAY [0..1023] OF CHAR;
  authHdr: ARRAY [0..2047] OF CHAR;
  jwt: ARRAY [0..2047] OF CHAR;
  sha: ARRAY [0..64] OF CHAR;
  excl: ARRAY [0..15] OF CHAR;
  skey: ARRAY [0..6] OF CHAR;
  ctype: ARRAY [0..31] OF CHAR;
  rmode: ARRAY [0..1] OF CHAR;
  rc, vl, fh, fileSize, bytesRead: INTEGER;
  jwtLen: CARDINAL;
  bodyPtr: ADDRESS;
  resp: ResponsePtr;
  st: HTTPClient.Status;
  body: ARRAY [0..4095] OF CHAR;
  bodyLen: INTEGER;
  bst: Buffers.Status;
BEGIN
  GetName(name);
  GetVersion(ver);
  IF Length(name) = 0 THEN
    WriteString("m2pkg: no package name in manifest"); WriteLn;
    RAISE RegistryError
  END;
  IF Length(ver) = 0 THEN
    WriteString("m2pkg: no version in manifest"); WriteLn;
    RAISE RegistryError
  END;
  IF IsValid(ver) = 0 THEN
    WriteString("m2pkg: version '"); WriteString(ver);
    WriteString("' is not valid semver (expected X.Y.Z)"); WriteLn;
    RAISE RegistryError
  END;
  IF Length(token) = 0 THEN
    WriteString("m2pkg: no auth token (set M2PKG_TOKEN)"); WriteLn;
    RAISE RegistryError
  END;

  (* Create tarball in cache dir *)
  GetCacheDir(cacheDir);
  rc := m2sys_mkdir_p(ADR(cacheDir));
  Assign(name, tarName);
  Concat(tarName, "-", tmp1); Assign(tmp1, tarName);
  Concat(tarName, ver, tmp1); Assign(tmp1, tarName);
  Concat(tarName, ".tar", tmp1); Assign(tmp1, tarName);
  m2sys_join_path(ADR(cacheDir), ADR(tarName), ADR(tarPath), 1024);

  Assign("target", excl);
  Assign(".", tmp1);
  rc := m2sys_tar_create_ex(ADR(tarPath), ADR(tmp1), ADR(excl));
  IF rc # 0 THEN
    WriteString("m2pkg: failed to create tarball"); WriteLn;
    RAISE RegistryError
  END;

  (* Read tarball into memory *)
  fileSize := m2sys_file_size(ADR(tarPath));
  IF fileSize <= 0 THEN
    WriteString("m2pkg: tarball is empty or unreadable"); WriteLn;
    RAISE RegistryError
  END;
  ALLOCATE(bodyPtr, fileSize);
  IF bodyPtr = NIL THEN
    WriteString("m2pkg: out of memory reading tarball"); WriteLn;
    RAISE RegistryError
  END;
  Assign("r", rmode);
  fh := m2sys_fopen(ADR(tarPath), ADR(rmode));
  IF fh < 0 THEN
    DEALLOCATE(bodyPtr, fileSize);
    WriteString("m2pkg: failed to open tarball"); WriteLn;
    RAISE RegistryError
  END;
  bytesRead := m2sys_fread_bytes(fh, bodyPtr, fileSize);
  rc := m2sys_fclose(fh);

  (* Build API URL: <url>/api/v1/packages/<name>/<version> *)
  Assign(url, apiUrl);
  Concat(apiUrl, "/api/v1/packages/", tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, name, tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, "/", tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, ver, tmp1); Assign(tmp1, apiUrl);

  (* Sign JWT from hex secret *)
  IF Auth.QuickSignHS256(token, "m2pkg", 300, jwt, jwtLen) # Auth.OK THEN
    WriteString("m2pkg: invalid auth token (expected 64-char hex secret)"); WriteLn;
    DEALLOCATE(bodyPtr, fileSize);
    RAISE RegistryError
  END;
  Assign("Bearer ", authHdr);
  Concat(authHdr, jwt, tmp1); Assign(tmp1, authHdr);

  (* PUT via native HTTP *)
  Assign("application/octet-stream", ctype);
  st := SyncPut(apiUrl, mInsecure, bodyPtr, bytesRead, ctype, authHdr, resp);
  DEALLOCATE(bodyPtr, fileSize);

  (* Clean up temp tarball *)
  rc := m2sys_remove_file(ADR(tarPath));

  IF st # HTTPClient.OK THEN
    WriteString("m2pkg: publish failed — could not connect to registry"); WriteLn;
    IF resp # NIL THEN FreeResponse(resp) END;
    RAISE RegistryError
  END;

  IF (resp # NIL) AND (resp^.statusCode # 200) THEN
    HttpErrorMsg(resp^.statusCode, name, ver);
    FreeResponse(resp);
    RAISE RegistryError
  END;

  (* Parse response body for sha256 confirmation *)
  sha[0] := 0C;
  IF (resp # NIL) AND (resp^.body # NIL) THEN
    bodyLen := Buffers.Length(resp^.body);
    IF bodyLen > 4095 THEN bodyLen := 4095 END;
    IF bodyLen > 0 THEN
      bst := Buffers.CopyOut(resp^.body, 0, bodyLen, body);
      body[bodyLen] := 0C;
      Assign("sha256", skey);
      vl := ExtractJsonValue(body, skey, sha)
    END
  END;
  IF resp # NIL THEN FreeResponse(resp) END;

  WriteString("m2pkg: published "); WriteString(name);
  WriteString(" "); WriteString(ver);
  WriteString(" to "); WriteString(url);
  IF Length(sha) > 0 THEN
    WriteString(" (sha256: "); WriteString(sha); WriteString(")")
  END;
  WriteLn
END PublishRemote;

PROCEDURE FetchVersions(url: ARRAY OF CHAR; name: ARRAY OF CHAR;
                        VAR verListBuf: ARRAY OF CHAR): INTEGER;
VAR
  apiUrl: ARRAY [0..1023] OF CHAR;
  resp: ResponsePtr;
  st: HTTPClient.Status;
  body: ARRAY [0..8191] OF CHAR;
  bodyLen, count: INTEGER;
  bst: Buffers.Status;
  vkey: ARRAY [0..8] OF CHAR;
BEGIN
  verListBuf[0] := 0C;

  (* Build API URL: <url>/api/v1/packages/<name>/versions *)
  Assign(url, apiUrl);
  Concat(apiUrl, "/api/v1/packages/", tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, name, tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, "/versions", tmp1); Assign(tmp1, apiUrl);

  st := SyncGet(apiUrl, mInsecure, resp);
  IF st # HTTPClient.OK THEN RETURN 0 END;
  IF resp = NIL THEN RETURN 0 END;

  IF resp^.statusCode # 200 THEN
    HttpErrorMsg(resp^.statusCode, name, "");
    FreeResponse(resp);
    RETURN 0
  END;

  bodyLen := Buffers.Length(resp^.body);
  IF bodyLen > 8191 THEN bodyLen := 8191 END;
  IF bodyLen > 0 THEN
    bst := Buffers.CopyOut(resp^.body, 0, bodyLen, body)
  END;
  body[bodyLen] := 0C;
  FreeResponse(resp);

  (* Parse JSON: {"name":"...","versions":["1.0.0","1.1.0",...]} *)
  Assign("versions", vkey);
  count := ExtractJsonArray(body, vkey, verListBuf);
  RETURN count
END FetchVersions;

PROCEDURE ResolveRangeRemote(url: ARRAY OF CHAR; name: ARRAY OF CHAR;
                             rangeSpec: ARRAY OF CHAR;
                             VAR resolvedVer: ARRAY OF CHAR;
                             VAR sha: ARRAY OF CHAR): INTEGER;
VAR
  verList: ARRAY [0..8191] OF CHAR;
  entry: ARRAY [0..63] OF CHAR;
  curVer, bestVer: Version;
  count, pos, epos, found, rc, ei: INTEGER;
  verKey: ARRAY [0..7] OF CHAR;
  sKey: ARRAY [0..6] OF CHAR;
  latestUrl: ARRAY [0..1023] OF CHAR;
  resp: ResponsePtr;
  st: HTTPClient.Status;
  body: ARRAY [0..4095] OF CHAR;
  bodyLen, vl: INTEGER;
  bst: Buffers.Status;
BEGIN
  resolvedVer[0] := 0C;
  sha[0] := 0C;

  count := FetchVersions(url, name, verList);
  IF count = 0 THEN RETURN -1 END;

  (* Scan versions, pick best matching range *)
  found := 0;
  bestVer.major := -1; bestVer.minor := 0; bestVer.patch := 0;
  pos := 0;
  WHILE pos <= Length(verList) DO
    epos := pos;
    WHILE (epos < Length(verList)) AND (verList[epos] # 12C) AND
          (verList[epos] # 0C) DO
      INC(epos)
    END;
    IF epos > pos THEN
      ei := 0;
      WHILE pos + ei < epos DO
        entry[ei] := verList[pos + ei]; INC(ei)
      END;
      entry[ei] := 0C;

      IF Parse(entry, curVer) = 0 THEN
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

  ToString(bestVer, resolvedVer);

  (* Get sha256 from /api/v1/packages/<name>/latest or index *)
  (* Try fetching index to get sha *)
  Assign(url, latestUrl);
  Concat(latestUrl, "/api/v1/packages/", tmp1); Assign(tmp1, latestUrl);
  Concat(latestUrl, name, tmp1); Assign(tmp1, latestUrl);
  Concat(latestUrl, "/latest", tmp1); Assign(tmp1, latestUrl);

  st := SyncGet(latestUrl, mInsecure, resp);
  IF (st = HTTPClient.OK) AND (resp # NIL) AND (resp^.statusCode = 200) THEN
    bodyLen := Buffers.Length(resp^.body);
    IF bodyLen > 4095 THEN bodyLen := 4095 END;
    IF bodyLen > 0 THEN
      bst := Buffers.CopyOut(resp^.body, 0, bodyLen, body)
    END;
    body[bodyLen] := 0C;
    FreeResponse(resp);
    Assign("sha256", sKey);
    vl := ExtractJsonValue(body, sKey, sha)
  ELSE
    IF resp # NIL THEN FreeResponse(resp) END
  END;

  RETURN 0
END ResolveRangeRemote;

PROCEDURE SearchRemote(url: ARRAY OF CHAR; query: ARRAY OF CHAR);
VAR
  apiUrl: ARRAY [0..1023] OF CHAR;
  resp: ResponsePtr;
  st: HTTPClient.Status;
  body: ARRAY [0..8191] OF CHAR;
  bodyLen: INTEGER;
  bst: Buffers.Status;
BEGIN
  (* Build API URL: <url>/api/v1/search/<query> *)
  Assign(url, apiUrl);
  Concat(apiUrl, "/api/v1/search/", tmp1); Assign(tmp1, apiUrl);
  Concat(apiUrl, query, tmp1); Assign(tmp1, apiUrl);

  st := SyncGet(apiUrl, mInsecure, resp);
  IF st # HTTPClient.OK THEN
    WriteString("m2pkg: search failed — could not connect to registry"); WriteLn;
    RAISE RegistryError
  END;
  IF resp = NIL THEN
    WriteString("m2pkg: search failed — no response"); WriteLn;
    RAISE RegistryError
  END;

  IF resp^.statusCode # 200 THEN
    HttpErrorMsg(resp^.statusCode, query, "");
    FreeResponse(resp);
    RAISE RegistryError
  END;

  bodyLen := Buffers.Length(resp^.body);
  IF bodyLen > 8191 THEN bodyLen := 8191 END;
  IF bodyLen > 0 THEN
    bst := Buffers.CopyOut(resp^.body, 0, bodyLen, body)
  END;
  body[bodyLen] := 0C;
  FreeResponse(resp);

  WriteString("m2pkg: search results for '"); WriteString(query);
  WriteString("':"); WriteLn;
  ExtractSearchResults(body)
END SearchRemote;

PROCEDURE SetInsecure(mode: BOOLEAN);
BEGIN
  mInsecure := mode
END SetInsecure;

BEGIN
  mInsecure := FALSE
END Registry.
