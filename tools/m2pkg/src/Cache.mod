IMPLEMENTATION MODULE Cache;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Length, Concat;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_sha256_str, m2sys_file_exists, m2sys_home_dir,
               m2sys_join_path, m2sys_mkdir_p, m2sys_strlen,
               m2sys_copy_file, m2sys_is_dir;

PROCEDURE ComputeKey(name: ARRAY OF CHAR; version: ARRAY OF CHAR;
                     entrySha: ARRAY OF CHAR; VAR key: ARRAY OF CHAR);
VAR
  data: ARRAY [0..2047] OF CHAR;
  tmp: ARRAY [0..2047] OF CHAR;
  hex: ARRAY [0..64] OF CHAR;
  len: INTEGER;
BEGIN
  (* Key = SHA-256 of "name:version:entrySha" *)
  Assign(name, data);
  Concat(data, ":", tmp);
  Assign(tmp, data);
  Concat(data, version, tmp);
  Assign(tmp, data);
  Concat(data, ":", tmp);
  Assign(tmp, data);
  Concat(data, entrySha, tmp);
  Assign(tmp, data);
  len := m2sys_strlen(ADR(data));
  m2sys_sha256_str(ADR(data), len, ADR(hex));
  Assign(hex, key)
END ComputeKey;

PROCEDURE CacheDirForKey(key: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
VAR
  home: ARRAY [0..255] OF CHAR;
  cachePath: ARRAY [0..511] OF CHAR;
BEGIN
  m2sys_home_dir(ADR(home), 256);
  m2sys_join_path(ADR(home), ADR(".m2pkg/cache"), ADR(cachePath), 512);
  m2sys_join_path(ADR(cachePath), ADR(key), ADR(out), 512)
END CacheDirForKey;

PROCEDURE IsCached(key: ARRAY OF CHAR): INTEGER;
VAR
  dir: ARRAY [0..511] OF CHAR;
  artifact: ARRAY [0..511] OF CHAR;
BEGIN
  CacheDirForKey(key, dir);
  m2sys_join_path(ADR(dir), ADR("artifact"), ADR(artifact), 512);
  RETURN m2sys_file_exists(ADR(artifact))
END IsCached;

PROCEDURE StoreResult(key: ARRAY OF CHAR; artifactPath: ARRAY OF CHAR): INTEGER;
VAR
  dir: ARRAY [0..511] OF CHAR;
  dest: ARRAY [0..511] OF CHAR;
  rc: INTEGER;
BEGIN
  CacheDirForKey(key, dir);
  rc := m2sys_mkdir_p(ADR(dir));
  IF rc # 0 THEN RETURN -1 END;
  m2sys_join_path(ADR(dir), ADR("artifact"), ADR(dest), 512);
  RETURN m2sys_copy_file(ADR(artifactPath), ADR(dest))
END StoreResult;

PROCEDURE LookupResult(key: ARRAY OF CHAR; destPath: ARRAY OF CHAR): INTEGER;
VAR
  dir: ARRAY [0..511] OF CHAR;
  src: ARRAY [0..511] OF CHAR;
BEGIN
  CacheDirForKey(key, dir);
  m2sys_join_path(ADR(dir), ADR("artifact"), ADR(src), 512);
  IF m2sys_file_exists(ADR(src)) = 0 THEN RETURN -1 END;
  RETURN m2sys_copy_file(ADR(src), ADR(destPath))
END LookupResult;

END Cache.
