IMPLEMENTATION MODULE Sockets;

FROM SYSTEM IMPORT ADR, BYTE;
FROM SocketsBridge IMPORT
     m2_socket, m2_close, m2_shutdown,
     m2_bind_any, m2_listen, m2_accept,
     m2_connect_host_port,
     m2_send, m2_recv,
     m2_sendto, m2_recvfrom,
     m2_set_multicast, m2_set_broadcast,
     m2_set_nonblocking, m2_set_reuseaddr,
     m2_errno, m2_strerror;

(* ── Errno constants (POSIX, portable Linux/macOS) ──── *)

CONST
  EAGAIN      = 35;   (* macOS; Linux = 11 — handled below *)
  EWOULDBLOCK = 35;   (* macOS; Linux = 11 *)
  ECONNRESET  = 54;   (* macOS; Linux = 104 *)

(* We don't hard-code errno values; we check the bridge at runtime. *)

(* ── Internal: map bridge return + errno to Status ──── *)

PROCEDURE MapError(): Status;
VAR e: INTEGER;
BEGIN
  e := m2_errno();
  (* EAGAIN/EWOULDBLOCK: 35 on macOS, 11 on Linux *)
  IF (e = 35) OR (e = 11) THEN RETURN WouldBlock END;
  RETURN SysError
END MapError;

(* ── Lifecycle ──────────────────────────────────────── *)

PROCEDURE SocketCreate(family, socktype: INTEGER;
                       VAR out: Socket): Status;
VAR fd: INTEGER;
BEGIN
  IF (family # AF_INET) THEN out := InvalidSocket; RETURN Invalid END;
  IF (socktype # SOCK_STREAM) AND (socktype # SOCK_DGRAM) THEN
    out := InvalidSocket; RETURN Invalid
  END;
  fd := m2_socket(family, socktype);
  IF fd < 0 THEN out := InvalidSocket; RETURN MapError() END;
  out := fd;
  RETURN OK
END SocketCreate;

PROCEDURE CloseSocket(s: Socket): Status;
BEGIN
  IF s = InvalidSocket THEN RETURN OK END;
  IF m2_close(s) < 0 THEN RETURN MapError() END;
  RETURN OK
END CloseSocket;

PROCEDURE Shutdown(s: Socket; how: INTEGER): Status;
BEGIN
  IF s = InvalidSocket THEN RETURN Invalid END;
  IF (how < SHUT_RD) OR (how > SHUT_RDWR) THEN RETURN Invalid END;
  IF m2_shutdown(s, how) < 0 THEN RETURN MapError() END;
  RETURN OK
END Shutdown;

(* ── Server ─────────────────────────────────────────── *)

PROCEDURE Bind(s: Socket; port: CARDINAL): Status;
VAR rc: INTEGER;
BEGIN
  IF s = InvalidSocket THEN RETURN Invalid END;
  (* Set SO_REUSEADDR so quick restart doesn't fail *)
  rc := m2_set_reuseaddr(s, 1);
  IF m2_bind_any(s, INTEGER(port)) < 0 THEN RETURN MapError() END;
  RETURN OK
END Bind;

PROCEDURE Listen(s: Socket; backlog: INTEGER): Status;
BEGIN
  IF s = InvalidSocket THEN RETURN Invalid END;
  IF backlog < 1 THEN backlog := 8 END;
  IF m2_listen(s, backlog) < 0 THEN RETURN MapError() END;
  RETURN OK
END Listen;

PROCEDURE Accept(s: Socket;
                 VAR outClient: Socket;
                 VAR outPeer: SockAddr): Status;
VAR fd, fam, pt: INTEGER;
    addr4: ARRAY [0..3] OF BYTE;
    rc: INTEGER;
BEGIN
  IF s = InvalidSocket THEN
    outClient := InvalidSocket; RETURN Invalid
  END;
  rc := m2_accept(s, fd, fam, pt, ADR(addr4));
  IF rc < 0 THEN
    outClient := InvalidSocket; RETURN MapError()
  END;
  outClient := fd;
  outPeer.family := fam;
  outPeer.port := CARDINAL(pt);
  outPeer.addrV4[0] := addr4[0];
  outPeer.addrV4[1] := addr4[1];
  outPeer.addrV4[2] := addr4[2];
  outPeer.addrV4[3] := addr4[3];
  RETURN OK
END Accept;

(* ── Client ─────────────────────────────────────────── *)

PROCEDURE Connect(s: Socket;
                  host: ARRAY OF CHAR;
                  port: CARDINAL): Status;
BEGIN
  IF s = InvalidSocket THEN RETURN Invalid END;
  IF m2_connect_host_port(s, ADR(host), INTEGER(port)) < 0 THEN
    RETURN MapError()
  END;
  RETURN OK
END Connect;

(* ── I/O ────────────────────────────────────────────── *)

PROCEDURE SendBytes(s: Socket;
                    VAR buf: ARRAY OF BYTE;
                    len: CARDINAL;
                    VAR sent: CARDINAL): Status;
VAR n: INTEGER;
BEGIN
  sent := 0;
  IF s = InvalidSocket THEN RETURN Invalid END;
  n := m2_send(s, ADR(buf), INTEGER(len));
  IF n < 0 THEN RETURN MapError() END;
  sent := CARDINAL(n);
  RETURN OK
END SendBytes;

PROCEDURE RecvBytes(s: Socket;
                    VAR buf: ARRAY OF BYTE;
                    max: CARDINAL;
                    VAR got: CARDINAL): Status;
VAR n: INTEGER;
BEGIN
  got := 0;
  IF s = InvalidSocket THEN RETURN Invalid END;
  n := m2_recv(s, ADR(buf), INTEGER(max));
  IF n < 0 THEN RETURN MapError() END;
  IF n = 0 THEN RETURN Closed END;
  got := CARDINAL(n);
  RETURN OK
END RecvBytes;

(* ── Convenience helpers (pure M2+ logic) ───────────── *)

PROCEDURE SendString(s: Socket; str: ARRAY OF CHAR): Status;
VAR len, n: INTEGER;
    total: CARDINAL;
    sent: CARDINAL;
    st: Status;
BEGIN
  (* Find string length *)
  len := 0;
  WHILE (len <= HIGH(str)) AND (str[len] # 0C) DO INC(len) END;
  IF len = 0 THEN RETURN OK END;

  (* Send in a loop until all bytes are out *)
  total := 0;
  WHILE total < CARDINAL(len) DO
    n := m2_send(s, ADR(str[total]), len - INTEGER(total));
    IF n < 0 THEN RETURN MapError() END;
    total := total + CARDINAL(n)
  END;
  RETURN OK
END SendString;

PROCEDURE RecvLine(s: Socket; VAR line: ARRAY OF CHAR): Status;
VAR ch: ARRAY [0..0] OF BYTE;
    n, pos: INTEGER;
    c: CHAR;
BEGIN
  pos := 0;
  LOOP
    n := m2_recv(s, ADR(ch), 1);
    IF n < 0 THEN
      (* Error — if we already have data, return it *)
      IF pos > 0 THEN
        IF pos <= HIGH(line) THEN line[pos] := 0C END;
        RETURN OK
      END;
      RETURN MapError()
    END;
    IF n = 0 THEN
      (* Peer closed *)
      IF pos > 0 THEN
        IF pos <= HIGH(line) THEN line[pos] := 0C END;
        RETURN OK
      END;
      RETURN Closed
    END;

    c := CHR(ORD(ch[0]));
    IF c = 12C THEN  (* LF = newline *)
      (* Strip trailing CR if present *)
      IF (pos > 0) AND (line[pos - 1] = 15C) THEN DEC(pos) END;
      IF pos <= HIGH(line) THEN line[pos] := 0C END;
      RETURN OK
    END;

    IF pos <= HIGH(line) THEN
      line[pos] := c;
      INC(pos)
    END
    (* If buffer full, keep reading until LF to consume the line *)
  END
END RecvLine;

(* ── UDP (datagram) I/O ──────────────────────────────── *)

PROCEDURE SendTo(s: Socket; VAR buf: ARRAY OF BYTE;
                 len: CARDINAL;
                 VAR addr: SockAddr): INTEGER;
VAR n: INTEGER;
BEGIN
  IF s = InvalidSocket THEN RETURN -1 END;
  n := m2_sendto(s, ADR(buf), INTEGER(len),
                 ORD(addr.addrV4[0]),
                 ORD(addr.addrV4[1]),
                 ORD(addr.addrV4[2]),
                 ORD(addr.addrV4[3]),
                 INTEGER(addr.port));
  RETURN n
END SendTo;

PROCEDURE RecvFrom(s: Socket; VAR buf: ARRAY OF BYTE;
                   maxLen: CARDINAL;
                   VAR addr: SockAddr): INTEGER;
VAR n, pt: INTEGER;
    addr4: ARRAY [0..3] OF BYTE;
BEGIN
  IF s = InvalidSocket THEN RETURN -1 END;
  pt := 0;
  n := m2_recvfrom(s, ADR(buf), INTEGER(maxLen), ADR(addr4), pt);
  IF n >= 0 THEN
    addr.family := AF_INET;
    addr.port := CARDINAL(pt);
    addr.addrV4[0] := addr4[0];
    addr.addrV4[1] := addr4[1];
    addr.addrV4[2] := addr4[2];
    addr.addrV4[3] := addr4[3]
  END;
  RETURN n
END RecvFrom;

PROCEDURE SetMulticastGroup(s: Socket; group: ARRAY OF CHAR;
                            join: BOOLEAN): Status;
VAR flag: INTEGER;
BEGIN
  IF s = InvalidSocket THEN RETURN Invalid END;
  IF join THEN flag := 1 ELSE flag := 0 END;
  IF m2_set_multicast(s, ADR(group), flag) < 0 THEN RETURN MapError() END;
  RETURN OK
END SetMulticastGroup;

PROCEDURE SetBroadcast(s: Socket; enable: BOOLEAN): Status;
VAR flag: INTEGER;
BEGIN
  IF s = InvalidSocket THEN RETURN Invalid END;
  IF enable THEN flag := 1 ELSE flag := 0 END;
  IF m2_set_broadcast(s, flag) < 0 THEN RETURN MapError() END;
  RETURN OK
END SetBroadcast;

(* ── Non-blocking ───────────────────────────────────── *)

PROCEDURE SetNonBlocking(s: Socket; enable: BOOLEAN): Status;
VAR flag: INTEGER;
BEGIN
  IF s = InvalidSocket THEN RETURN Invalid END;
  IF enable THEN flag := 1 ELSE flag := 0 END;
  IF m2_set_nonblocking(s, flag) < 0 THEN RETURN MapError() END;
  RETURN OK
END SetNonBlocking;

(* ── Error info ─────────────────────────────────────── *)

PROCEDURE GetLastErrno(): INTEGER;
BEGIN
  RETURN m2_errno()
END GetLastErrno;

PROCEDURE GetLastErrorText(VAR out: ARRAY OF CHAR);
BEGIN
  m2_strerror(m2_errno(), ADR(out), HIGH(out) + 1)
END GetLastErrorText;

END Sockets.
