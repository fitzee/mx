IMPLEMENTATION MODULE Resolver;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Length, Concat;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_file_exists, m2sys_is_dir;
FROM Manifest IMPORT DepCount, GetDepName, GetDepPath, IsDepLocal, GetDepVersion;
FROM Lockfile IMPORT WriteEnhanced, SetDepEntry, SetLockDepCount;
FROM Registry IMPORT Fetch, Lookup, LookupRange, FetchRemote;
FROM Manifest IMPORT GetRegistryURL;
FROM Semver IMPORT IsValid;

VAR
  tmp: ARRAY [0..511] OF CHAR;
  registryUrl: ARRAY [0..511] OF CHAR;

PROCEDURE Resolve;
VAR
  i, nd, rc: INTEGER;
  dname: ARRAY [0..63] OF CHAR;
  dpath: ARRAY [0..255] OF CHAR;
  dver: ARRAY [0..31] OF CHAR;
  resolvedVer: ARRAY [0..31] OF CHAR;
  mpath: ARRAY [0..511] OF CHAR;
  sha: ARRAY [0..64] OF CHAR;
  fetchPath: ARRAY [0..511] OF CHAR;
  isRange: INTEGER;
BEGIN
  nd := DepCount();
  SetLockDepCount(nd);

  (* Check if a global registry URL is configured *)
  GetRegistryURL(registryUrl);

  IF nd = 0 THEN
    WriteString("m2pkg: no dependencies to resolve"); WriteLn
  ELSE
    i := 0;
    WHILE i < nd DO
      GetDepName(i, dname);
      GetDepPath(i, dpath);

      IF IsDepLocal(i) = 1 THEN
        (* Local path dependency *)
        IF m2sys_is_dir(ADR(dpath)) = 0 THEN
          WriteString("m2pkg: dependency '");
          WriteString(dname);
          WriteString("' path not found: ");
          WriteString(dpath); WriteLn;
          RAISE ResolveError
        END;
        (* Check that dep has a m2.toml *)
        Assign(dpath, mpath);
        Concat(mpath, "/m2.toml", tmp);
        Assign(tmp, mpath);
        IF m2sys_file_exists(ADR(mpath)) = 0 THEN
          WriteString("m2pkg: dependency '");
          WriteString(dname);
          WriteString("' has no m2.toml at ");
          WriteString(mpath); WriteLn;
          RAISE ResolveError
        END;
        WriteString("m2pkg: resolved "); WriteString(dname);
        WriteString(" -> "); WriteString(dpath); WriteLn;
        SetDepEntry(i, dname, "", "local", "", dpath)
      ELSE
        (* Registry dependency — version spec is in dpath *)
        GetDepVersion(i, dver);

        (* Check if dver starts with https:// — remote dep *)
        IF (Length(dver) > 8) AND (dver[0] = 'h') AND (dver[1] = 't') AND
           (dver[2] = 't') AND (dver[3] = 'p') THEN
          FetchRemote(registryUrl, dname, dver, fetchPath);
          SetDepEntry(i, dname, dver, "registry", "", fetchPath)
        ELSE
          (* Check if it's a range (starts with ^, ~, >=) vs exact *)
          isRange := 0;
          IF (dver[0] = '^') OR (dver[0] = '~') THEN
            isRange := 1
          ELSIF (dver[0] = '>') AND (Length(dver) > 1) AND (dver[1] = '=') THEN
            isRange := 1
          END;

          IF isRange = 1 THEN
            rc := LookupRange(dname, dver, resolvedVer, sha);
            IF rc # 0 THEN
              (* Try remote if registry URL configured *)
              IF Length(registryUrl) > 0 THEN
                FetchRemote(registryUrl, dname, dver, fetchPath);
                SetDepEntry(i, dname, dver, "registry", "", fetchPath)
              ELSE
                WriteString("m2pkg: no matching version for ");
                WriteString(dname); WriteString(" "); WriteString(dver); WriteLn;
                RAISE ResolveError
              END
            ELSE
              Fetch(dname, resolvedVer, fetchPath);
              WriteString("m2pkg: resolved "); WriteString(dname);
              WriteString("@"); WriteString(dver);
              WriteString(" -> "); WriteString(resolvedVer);
              WriteString(" -> "); WriteString(fetchPath); WriteLn;
              SetDepEntry(i, dname, resolvedVer, "registry", sha, fetchPath)
            END
          ELSE
            (* Exact version *)
            rc := Lookup(dname, dver, sha);
            IF rc # 0 THEN
              (* Try remote if registry URL configured *)
              IF Length(registryUrl) > 0 THEN
                FetchRemote(registryUrl, dname, dver, fetchPath);
                SetDepEntry(i, dname, dver, "registry", "", fetchPath)
              ELSE
                WriteString("m2pkg: package not found in registry: ");
                WriteString(dname); WriteString(" "); WriteString(dver); WriteLn;
                RAISE ResolveError
              END
            ELSE
              Fetch(dname, dver, fetchPath);
              WriteString("m2pkg: resolved "); WriteString(dname);
              WriteString("@"); WriteString(dver);
              WriteString(" -> "); WriteString(fetchPath); WriteLn;
              SetDepEntry(i, dname, dver, "registry", sha, fetchPath)
            END
          END
        END
      END;
      INC(i)
    END
  END;

  (* Write enhanced lockfile *)
  WriteEnhanced("m2.lock");
  WriteString("m2pkg: wrote m2.lock"); WriteLn
END Resolve;

END Resolver.
