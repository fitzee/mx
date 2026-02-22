MODULE h2s_minimal;

  (* Minimal HTTP/2 server example.
     Creates a server, adds a single route, and starts listening. *)

  FROM SYSTEM IMPORT ADDRESS;
  FROM InOut IMPORT WriteString, WriteLn;
  FROM Http2ServerTypes IMPORT ServerOpts, Status, Request, Response,
                                InitDefaultOpts;
  FROM Http2Server IMPORT Server, Create, AddRoute, Start, Destroy;
  FROM ByteBuf IMPORT AppendChars;

  PROCEDURE HelloHandler(VAR req: Request; VAR resp: Response;
                         ctx: ADDRESS);
  BEGIN
    resp.status := 200;
    AppendChars(resp.body, "Hello from Modula-2 HTTP/2!");
    resp.bodyLen := 26;
  END HelloHandler;

VAR
  opts: ServerOpts;
  srv: Server;
  st: Status;
BEGIN
  InitDefaultOpts(opts);
  opts.port := 8443;
  opts.certPath := "server.crt";
  opts.keyPath := "server.key";

  st := Create(opts, srv);
  IF st # OK THEN
    WriteString("Failed to create server"); WriteLn;
    HALT;
  END;

  IF NOT AddRoute(srv, "GET", "/hello", HelloHandler, NIL) THEN
    WriteString("Failed to add route"); WriteLn;
    HALT;
  END;

  WriteString("Starting HTTP/2 server on port 8443..."); WriteLn;
  st := Start(srv);

  st := Destroy(srv);
END h2s_minimal.
