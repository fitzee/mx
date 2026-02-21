IMPLEMENTATION MODULE Poller;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM PollerBridge IMPORT m2_poller_create, m2_poller_destroy,
                         m2_poller_add, m2_poller_mod, m2_poller_del,
                         m2_poller_wait, m2_now_ms;

PROCEDURE Create(VAR out: Poller): Status;
VAR h: INTEGER;
BEGIN
  h := m2_poller_create();
  IF h < 0 THEN
    out := -1;
    RETURN SysError
  END;
  out := h;
  RETURN OK
END Create;

PROCEDURE Destroy(VAR p: Poller): Status;
BEGIN
  IF p < 0 THEN RETURN Invalid END;
  m2_poller_destroy(p);
  p := -1;
  RETURN OK
END Destroy;

PROCEDURE Add(p: Poller; fd, events: INTEGER): Status;
BEGIN
  IF p < 0 THEN RETURN Invalid END;
  IF m2_poller_add(p, fd, events) < 0 THEN
    RETURN SysError
  END;
  RETURN OK
END Add;

PROCEDURE Modify(p: Poller; fd, events: INTEGER): Status;
BEGIN
  IF p < 0 THEN RETURN Invalid END;
  IF m2_poller_mod(p, fd, events) < 0 THEN
    RETURN SysError
  END;
  RETURN OK
END Modify;

PROCEDURE Remove(p: Poller; fd: INTEGER): Status;
BEGIN
  IF p < 0 THEN RETURN Invalid END;
  IF m2_poller_del(p, fd) < 0 THEN
    RETURN SysError
  END;
  RETURN OK
END Remove;

PROCEDURE Wait(p: Poller; timeoutMs: INTEGER;
               VAR buf: EventBuf;
               VAR count: INTEGER): Status;
VAR n: INTEGER;
BEGIN
  IF p < 0 THEN
    count := 0;
    RETURN Invalid
  END;
  n := m2_poller_wait(p, timeoutMs, ADR(buf), MaxEvents);
  IF n < 0 THEN
    count := 0;
    RETURN SysError
  END;
  count := n;
  RETURN OK
END Wait;

PROCEDURE NowMs(): INTEGER;
BEGIN
  RETURN m2_now_ms()
END NowMs;

END Poller.
