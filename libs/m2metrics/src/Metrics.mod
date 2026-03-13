IMPLEMENTATION MODULE Metrics;

FROM SYSTEM IMPORT ADR;
FROM MetricsBridge IMPORT m2_metrics_snapshot;

PROCEDURE Snapshot(VAR snap: SysSnapshot);
VAR rc: INTEGER;
BEGIN
  rc := m2_metrics_snapshot(ADR(snap))
END Snapshot;

END Metrics.
