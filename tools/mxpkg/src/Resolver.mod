IMPLEMENTATION MODULE Resolver;

FROM SYSTEM IMPORT ADR;
FROM Strings IMPORT Assign, Length, Concat, CompareStr;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_file_exists, m2sys_is_dir, m2sys_getenv;
FROM Manifest IMPORT DepCount, GetDepName, GetDepPath, IsDepLocal, GetDepVersion,
                     IsDepURL, GetDepURL, Read, Clear;
FROM Lockfile IMPORT WriteEnhanced, SetDepEntry, SetLockDepCount, SetDepURL,
                     GetDepResolvedPath, GetDepSource, GetDepLockName, LockDepCount;
FROM Registry IMPORT Fetch, Lookup, LookupRange, FetchRemote, FetchLatest,
                     SetInsecure, ResolveRangeRemote;
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
  depUrl: ARRAY [0..511] OF CHAR;
  isRange: INTEGER;
  envBuf: ARRAY [0..15] OF CHAR;
  bfsIdx: INTEGER;
  tdCount, ti: INTEGER;
  tdName: ARRAY [0..63] OF CHAR;
  tdVer: ARRAY [0..31] OF CHAR;
  srcBuf: ARRAY [0..15] OF CHAR;
  alreadyResolved: INTEGER;
  k: INTEGER;
  lockName: ARRAY [0..63] OF CHAR;
BEGIN
  nd := DepCount();
  SetLockDepCount(nd);

  (* Check if a global registry URL is configured *)
  GetRegistryURL(registryUrl);

  (* Check MXPKG_INSECURE env var for self-signed cert support *)
  m2sys_getenv(ADR("MXPKG_INSECURE"), ADR(envBuf), 16);
  IF (envBuf[0] = '1') OR (envBuf[0] = 't') THEN
    SetInsecure(TRUE)
  ELSE
    SetInsecure(FALSE)
  END;

  IF nd = 0 THEN
    WriteString("mxpkg: no dependencies to resolve"); WriteLn
  ELSE
    i := 0;
    WHILE i < nd DO
      GetDepName(i, dname);
      GetDepPath(i, dpath);

      IF IsDepLocal(i) = 1 THEN
        (* Local path dependency *)
        IF m2sys_is_dir(ADR(dpath)) = 0 THEN
          WriteString("mxpkg: dependency '");
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
          WriteString("mxpkg: dependency '");
          WriteString(dname);
          WriteString("' has no m2.toml at ");
          WriteString(mpath); WriteLn;
          RAISE ResolveError
        END;
        WriteString("mxpkg: resolved "); WriteString(dname);
        WriteString(" -> "); WriteString(dpath); WriteLn;
        SetDepEntry(i, dname, "", "local", "", dpath)
      ELSIF IsDepURL(i) = 1 THEN
        (* URL dependency — fetch from remote server *)
        GetDepURL(i, depUrl);
        GetDepVersion(i, dver);
        SetDepURL(i, depUrl);

        IF Length(dver) = 0 THEN
          (* No pinned version — fetch latest from server *)
          rc := FetchLatest(depUrl, dname, resolvedVer, sha);
          IF rc # 0 THEN
            WriteString("mxpkg: failed to fetch latest version of ");
            WriteString(dname); WriteString(" from "); WriteString(depUrl); WriteLn;
            RAISE ResolveError
          END;
          FetchRemote(depUrl, dname, resolvedVer, fetchPath);
          WriteString("mxpkg: resolved "); WriteString(dname);
          WriteString(" (latest) -> "); WriteString(resolvedVer);
          WriteString(" -> "); WriteString(fetchPath); WriteLn;
          SetDepEntry(i, dname, resolvedVer, "remote", sha, fetchPath)
        ELSIF (dver[0] = '^') OR (dver[0] = '~') OR
              ((dver[0] = '>') AND (Length(dver) > 1) AND (dver[1] = '=')) THEN
          (* Range spec — resolve via /versions endpoint *)
          rc := ResolveRangeRemote(depUrl, dname, dver, resolvedVer, sha);
          IF rc # 0 THEN
            WriteString("mxpkg: no matching version for ");
            WriteString(dname); WriteString(" "); WriteString(dver);
            WriteString(" on "); WriteString(depUrl); WriteLn;
            RAISE ResolveError
          END;
          FetchRemote(depUrl, dname, resolvedVer, fetchPath);
          WriteString("mxpkg: resolved "); WriteString(dname);
          WriteString("@"); WriteString(dver);
          WriteString(" -> "); WriteString(resolvedVer);
          WriteString(" -> "); WriteString(fetchPath); WriteLn;
          SetDepEntry(i, dname, resolvedVer, "remote", sha, fetchPath)
        ELSE
          (* Pinned exact version *)
          FetchRemote(depUrl, dname, dver, fetchPath);
          WriteString("mxpkg: resolved "); WriteString(dname);
          WriteString("@"); WriteString(dver);
          WriteString(" -> "); WriteString(fetchPath); WriteLn;
          SetDepEntry(i, dname, dver, "remote", "", fetchPath)
        END
      ELSE
        (* Registry dependency — version spec is in dpath *)
        GetDepVersion(i, dver);

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
              rc := ResolveRangeRemote(registryUrl, dname, dver, resolvedVer, sha);
              IF rc # 0 THEN
                WriteString("mxpkg: no matching version for ");
                WriteString(dname); WriteString(" "); WriteString(dver); WriteLn;
                RAISE ResolveError
              END;
              FetchRemote(registryUrl, dname, resolvedVer, fetchPath);
              WriteString("mxpkg: resolved "); WriteString(dname);
              WriteString("@"); WriteString(dver);
              WriteString(" -> "); WriteString(resolvedVer);
              WriteString(" -> "); WriteString(fetchPath); WriteLn;
              SetDepEntry(i, dname, resolvedVer, "registry", sha, fetchPath)
            ELSE
              WriteString("mxpkg: no matching version for ");
              WriteString(dname); WriteString(" "); WriteString(dver); WriteLn;
              RAISE ResolveError
            END
          ELSE
            Fetch(dname, resolvedVer, fetchPath);
            WriteString("mxpkg: resolved "); WriteString(dname);
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
              WriteString("mxpkg: package not found in registry: ");
              WriteString(dname); WriteString(" "); WriteString(dver); WriteLn;
              RAISE ResolveError
            END
          ELSE
            Fetch(dname, dver, fetchPath);
            WriteString("mxpkg: resolved "); WriteString(dname);
            WriteString("@"); WriteString(dver);
            WriteString(" -> "); WriteString(fetchPath); WriteLn;
            SetDepEntry(i, dname, dver, "registry", sha, fetchPath)
          END
        END
      END;
      INC(i)
    END
  END;

  (* --- BFS for transitive remote deps --- *)
  IF nd > 0 THEN
    bfsIdx := 0;
    WHILE bfsIdx < nd DO
      GetDepSource(bfsIdx, srcBuf);
      IF CompareStr(srcBuf, "local") # 0 THEN
        (* Non-local dep — read its manifest to discover its deps *)
        GetDepResolvedPath(bfsIdx, fetchPath);
        IF Length(fetchPath) > 0 THEN
          Assign(fetchPath, mpath);
          Concat(mpath, "/m2.toml", tmp);
          Assign(tmp, mpath);
          Clear;
          Read(mpath);
          tdCount := DepCount();
          ti := 0;
          WHILE ti < tdCount DO
            GetDepName(ti, tdName);
            GetDepVersion(ti, tdVer);
            (* Check if already resolved *)
            alreadyResolved := 0;
            k := 0;
            WHILE k < nd DO
              GetDepLockName(k, lockName);
              IF CompareStr(tdName, lockName) = 0 THEN
                alreadyResolved := 1
              END;
              INC(k)
            END;
            IF alreadyResolved = 0 THEN
              (* Transitive dep not yet in lockfile — resolve it *)
              IF (IsDepLocal(ti) = 1) OR (Length(tdVer) = 0) THEN
                (* Path dep in published package or no version — fetch latest from registry *)
                IF Length(registryUrl) > 0 THEN
                  rc := FetchLatest(registryUrl, tdName, resolvedVer, sha);
                  IF rc = 0 THEN
                    FetchRemote(registryUrl, tdName, resolvedVer, fetchPath);
                    WriteString("mxpkg: resolved transitive "); WriteString(tdName);
                    WriteString(" (latest) -> "); WriteString(resolvedVer);
                    WriteString(" -> "); WriteString(fetchPath); WriteLn;
                    SetDepEntry(nd, tdName, resolvedVer, "registry", sha, fetchPath);
                    INC(nd)
                  ELSE
                    WriteString("mxpkg: warning: cannot resolve transitive dep ");
                    WriteString(tdName); WriteLn
                  END
                END
              ELSIF (tdVer[0] = '^') OR (tdVer[0] = '~') OR
                    ((tdVer[0] = '>') AND (Length(tdVer) > 1) AND (tdVer[1] = '=')) THEN
                (* Range spec *)
                rc := LookupRange(tdName, tdVer, resolvedVer, sha);
                IF rc # 0 THEN
                  IF Length(registryUrl) > 0 THEN
                    rc := ResolveRangeRemote(registryUrl, tdName, tdVer, resolvedVer, sha);
                    IF rc = 0 THEN
                      FetchRemote(registryUrl, tdName, resolvedVer, fetchPath);
                      WriteString("mxpkg: resolved transitive "); WriteString(tdName);
                      WriteString("@"); WriteString(tdVer);
                      WriteString(" -> "); WriteString(resolvedVer);
                      WriteString(" -> "); WriteString(fetchPath); WriteLn;
                      SetDepEntry(nd, tdName, resolvedVer, "registry", sha, fetchPath);
                      INC(nd)
                    ELSE
                      WriteString("mxpkg: warning: cannot resolve transitive dep ");
                      WriteString(tdName); WriteString(" "); WriteString(tdVer); WriteLn
                    END
                  END
                ELSE
                  Fetch(tdName, resolvedVer, fetchPath);
                  WriteString("mxpkg: resolved transitive "); WriteString(tdName);
                  WriteString("@"); WriteString(tdVer);
                  WriteString(" -> "); WriteString(resolvedVer);
                  WriteString(" -> "); WriteString(fetchPath); WriteLn;
                  SetDepEntry(nd, tdName, resolvedVer, "registry", sha, fetchPath);
                  INC(nd)
                END
              ELSE
                (* Exact version *)
                rc := Lookup(tdName, tdVer, sha);
                IF rc # 0 THEN
                  IF Length(registryUrl) > 0 THEN
                    FetchRemote(registryUrl, tdName, tdVer, fetchPath);
                    WriteString("mxpkg: resolved transitive "); WriteString(tdName);
                    WriteString("@"); WriteString(tdVer);
                    WriteString(" -> "); WriteString(fetchPath); WriteLn;
                    SetDepEntry(nd, tdName, tdVer, "registry", "", fetchPath);
                    INC(nd)
                  ELSE
                    WriteString("mxpkg: warning: cannot resolve transitive dep ");
                    WriteString(tdName); WriteString(" "); WriteString(tdVer); WriteLn
                  END
                ELSE
                  Fetch(tdName, tdVer, fetchPath);
                  WriteString("mxpkg: resolved transitive "); WriteString(tdName);
                  WriteString("@"); WriteString(tdVer);
                  WriteString(" -> "); WriteString(fetchPath); WriteLn;
                  SetDepEntry(nd, tdName, tdVer, "registry", sha, fetchPath);
                  INC(nd)
                END
              END
            END;
            INC(ti)
          END
        END
      END;
      INC(bfsIdx)
    END;
    (* Restore main manifest *)
    Clear;
    Read("m2.toml");
    SetLockDepCount(nd)
  END;

  (* Write enhanced lockfile *)
  WriteEnhanced("m2.lock");
  WriteString("mxpkg: wrote m2.lock"); WriteLn
END Resolve;

END Resolver.
