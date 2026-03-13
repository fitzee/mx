# Metrics

Cross-platform system metrics: load average, memory, CPU time, and process RSS. Single call, no configuration, no external dependencies.

## Why Metrics?

Modula-2 has no built-in access to OS-level performance counters. Metrics provides a single `Snapshot` call that fills a record with nine system measurements, backed by platform-specific C code (macOS via Mach/sysctl, Linux via sysinfo/procfs). No initialization, no cleanup -- just call and read.

Used by pdc (Performance Data Collector) and any application that needs system telemetry.

## Types

### SysSnapshot

```modula2
TYPE
  SysSnapshot = RECORD
    load1:      LONGINT;   (* 1-min load avg x 1000  *)
    load5:      LONGINT;   (* 5-min load avg x 1000  *)
    load15:     LONGINT;   (* 15-min load avg x 1000 *)
    memTotalMB: LONGINT;   (* total physical RAM (MB) *)
    memFreeMB:  LONGINT;   (* free + inactive RAM (MB) *)
    cpuUserUs:  LONGINT;   (* user CPU time (us)      *)
    cpuSysUs:   LONGINT;   (* system CPU time (us)    *)
    rssKB:      LONGINT;   (* current RSS (KB)        *)
    maxRssKB:   LONGINT;   (* peak RSS (KB)           *)
  END;
```

All fields are LONGINT (signed 64-bit). Load averages are multiplied by 1000 to preserve three decimal places without floating point. CPU times are cumulative microseconds since process start.

| Field | Source (macOS) | Source (Linux) |
|-------|---------------|----------------|
| `load1/5/15` | `getloadavg()` | `getloadavg()` |
| `memTotalMB` | `sysctl(HW_MEMSIZE)` | `sysinfo.totalram` |
| `memFreeMB` | Mach `host_statistics64` (free + inactive) | `sysinfo.freeram + bufferram` |
| `cpuUserUs` | `getrusage(RUSAGE_SELF)` | `getrusage(RUSAGE_SELF)` |
| `cpuSysUs` | `getrusage(RUSAGE_SELF)` | `getrusage(RUSAGE_SELF)` |
| `rssKB` | Mach `task_info` | `/proc/self/status VmRSS` |
| `maxRssKB` | `getrusage` (bytes/1024) | `getrusage` (already KB) |

## Procedures

- `PROCEDURE Snapshot(VAR snap: SysSnapshot)`
  Fill `snap` with current system metrics. All nine fields are populated in a single call. Fields that cannot be read on the current platform are left as zero.

## Example

```modula2
FROM Metrics IMPORT SysSnapshot, Snapshot;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

VAR snap: SysSnapshot;

BEGIN
  Snapshot(snap);
  WriteString("load1="); WriteInt(snap.load1, 1); WriteLn;
  WriteString("rssKB="); WriteInt(snap.rssKB, 1); WriteLn;
  WriteString("memFreeMB="); WriteInt(snap.memFreeMB, 1); WriteLn
END
```

## Architecture

```
Metrics.def          High-level Modula-2 API (SysSnapshot, Snapshot)
  |
Metrics.mod          Delegates to FFI bridge via ADR(snap)
  |
MetricsBridge.def    FOR "C" declaration (ADDRESS-based FFI)
  |
metrics_bridge.c     Platform-specific C: getloadavg, sysctl/sysinfo, getrusage
```

No external library dependency. The C bridge uses only POSIX and OS headers.
