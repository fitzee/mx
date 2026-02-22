MODULE h2s_cli;

  (* HTTP/2 server with CLI argument parsing via m2Cli. *)

  FROM SYSTEM IMPORT ADDRESS;
  FROM InOut IMPORT WriteString, WriteLn, WriteCard;
  FROM Http2ServerTypes IMPORT ServerOpts, Status, Request, Response,
                                InitDefaultOpts;
  FROM Http2Server IMPORT Server, Create, AddRoute, AddMiddleware,
                          Start, Destroy;
  FROM Http2Middleware IMPORT LoggingMw;
  FROM Cli IMPORT AddOption, AddFlag, Parse, HasFlag, GetOption;
  FROM ByteBuf IMPORT AppendChars;

  PROCEDURE HelloHandler(VAR req: Request; VAR resp: Response;
                         ctx: ADDRESS);
  BEGIN
    resp.status := 200;
    AppendChars(resp.body, "Hello!");
    resp.bodyLen := 6;
  END HelloHandler;

  PROCEDURE HealthHandler(VAR req: Request; VAR resp: Response;
                          ctx: ADDRESS);
  BEGIN
    resp.status := 200;
    AppendChars(resp.body, "OK");
    resp.bodyLen := 2;
  END HealthHandler;

  PROCEDURE CopyStr(src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
  VAR
    i: CARDINAL;
  BEGIN
    i := 0;
    WHILE (i <= HIGH(src)) AND (i <= HIGH(dst)) AND (src[i] # 0C) DO
      dst[i] := src[i]; INC(i);
    END;
    IF i <= HIGH(dst) THEN dst[i] := 0C END;
  END CopyStr;

  PROCEDURE ParsePort(s: ARRAY OF CHAR): CARDINAL;
  VAR
    i, n: CARDINAL;
  BEGIN
    n := 0; i := 0;
    WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
      IF (s[i] >= "0") AND (s[i] <= "9") THEN
        n := n * 10 + (ORD(s[i]) - ORD("0"));
      END;
      INC(i);
    END;
    RETURN n;
  END ParsePort;

VAR
  opts: ServerOpts;
  srv: Server;
  st: Status;
  portStr: ARRAY [0..15] OF CHAR;
  certStr: ARRAY [0..255] OF CHAR;
  keyStr: ARRAY [0..255] OF CHAR;
  ok: BOOLEAN;
BEGIN
  AddOption("port", "p", "Listen port (default 8443)");
  AddOption("cert", "c", "TLS certificate PEM path");
  AddOption("key", "k", "TLS private key PEM path");
  AddFlag("help", "h", "Show help");

  ok := Parse();

  IF HasFlag("help") OR (NOT ok) THEN
    WriteString("Usage: h2s_cli --cert <path> --key <path> [--port <num>]");
    WriteLn;
    HALT;
  END;

  InitDefaultOpts(opts);

  IF GetOption("port", portStr) THEN
    opts.port := ParsePort(portStr);
  END;

  IF GetOption("cert", certStr) THEN
    CopyStr(certStr, opts.certPath);
  ELSE
    WriteString("Error: --cert is required"); WriteLn;
    HALT;
  END;

  IF GetOption("key", keyStr) THEN
    CopyStr(keyStr, opts.keyPath);
  ELSE
    WriteString("Error: --key is required"); WriteLn;
    HALT;
  END;

  st := Create(opts, srv);
  IF st # OK THEN
    WriteString("Failed to create server"); WriteLn;
    HALT;
  END;

  ok := AddRoute(srv, "GET", "/hello", HelloHandler, NIL);
  ok := AddRoute(srv, "GET", "/health", HealthHandler, NIL);

  WriteString("HTTP/2 server starting on port ");
  WriteCard(opts.port, 1);
  WriteLn;

  st := Start(srv);
  st := Destroy(srv);
END h2s_cli.
