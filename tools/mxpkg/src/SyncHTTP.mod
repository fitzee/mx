IMPLEMENTATION MODULE SyncHTTP;

FROM SYSTEM IMPORT ADR, ADDRESS;
FROM HTTPClient IMPORT ResponsePtr, Status, FreeResponse, SetSkipVerify;
IMPORT HTTPClient;
IMPORT H2Client;
FROM URI IMPORT URIRec;
IMPORT URI;
IMPORT EventLoop;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Fate, Result, GetResultIfSettled, GetFate;
IMPORT Promise;
IMPORT Buffers;
FROM Sys IMPORT m2sys_fopen, m2sys_fclose, m2sys_fwrite_bytes;

PROCEDURE IsHTTPS(VAR uri: URIRec): BOOLEAN;
BEGIN
  RETURN (uri.schemeLen = 5) AND
         (uri.scheme[0] = 'h') AND (uri.scheme[1] = 't') AND
         (uri.scheme[2] = 't') AND (uri.scheme[3] = 'p') AND
         (uri.scheme[4] = 's')
END IsHTTPS;

PROCEDURE SyncGet(VAR url: ARRAY OF CHAR;
                  skipVerify: BOOLEAN;
                  VAR resp: ResponsePtr): Status;
VAR
  uri: URIRec;
  ust: URI.Status;
  lp: EventLoop.Loop;
  sched: Scheduler;
  est: EventLoop.Status;
  future: Future;
  st: Status;
  settled: BOOLEAN;
  res: Result;
  pst: Promise.Status;
  running: BOOLEAN;
  useH2: BOOLEAN;
BEGIN
  resp := NIL;

  (* 1. Parse URL *)
  ust := URI.Parse(url, uri);
  IF ust # URI.OK THEN RETURN HTTPClient.Invalid END;

  useH2 := IsHTTPS(uri);

  (* 2. Create event loop *)
  est := EventLoop.Create(lp);
  IF est # EventLoop.OK THEN RETURN HTTPClient.ConnectFailed END;

  (* 3. Get scheduler *)
  sched := EventLoop.GetScheduler(lp);

  (* 4. Configure TLS verification *)
  IF skipVerify THEN
    SetSkipVerify(TRUE);
    H2Client.SetSkipVerify(TRUE)
  END;

  (* 5. Issue async GET — H2 for HTTPS, HTTP/1.1 for HTTP *)
  IF useH2 THEN
    st := H2Client.Get(lp, sched, uri, future)
  ELSE
    st := HTTPClient.Get(lp, sched, uri, future)
  END;
  IF st # HTTPClient.OK THEN
    IF skipVerify THEN
      SetSkipVerify(FALSE);
      H2Client.SetSkipVerify(FALSE)
    END;
    est := EventLoop.Destroy(lp);
    RETURN st
  END;

  (* 6. Run event loop until future settles *)
  LOOP
    running := EventLoop.RunOnce(lp);
    pst := GetResultIfSettled(future, settled, res);
    IF settled THEN EXIT END;
    IF NOT running THEN EXIT END
  END;

  (* 7. Extract result *)
  IF settled AND res.isOk THEN
    resp := res.v.ptr
  ELSE
    st := HTTPClient.ConnectFailed
  END;

  (* 8. Cleanup *)
  est := EventLoop.Destroy(lp);
  IF skipVerify THEN
    SetSkipVerify(FALSE);
    H2Client.SetSkipVerify(FALSE)
  END;

  IF resp # NIL THEN
    RETURN HTTPClient.OK
  ELSE
    RETURN st
  END
END SyncGet;

PROCEDURE SyncDownload(VAR url: ARRAY OF CHAR;
                       skipVerify: BOOLEAN;
                       VAR destPath: ARRAY OF CHAR;
                       VAR httpStatus: INTEGER): Status;
VAR
  resp: ResponsePtr;
  st: Status;
  fh, rc, bodyLen: INTEGER;
  wmode: ARRAY [0..1] OF CHAR;
BEGIN
  httpStatus := 0;

  (* 1. Do sync GET *)
  st := SyncGet(url, skipVerify, resp);
  IF st # HTTPClient.OK THEN RETURN st END;
  IF resp = NIL THEN RETURN HTTPClient.ConnectFailed END;

  (* 2. Record HTTP status *)
  httpStatus := resp^.statusCode;

  (* 3. Write body to file *)
  wmode[0] := 'w'; wmode[1] := 0C;
  fh := m2sys_fopen(ADR(destPath), ADR(wmode));
  IF fh < 0 THEN
    FreeResponse(resp);
    RETURN HTTPClient.Invalid
  END;

  bodyLen := Buffers.Length(resp^.body);
  IF bodyLen > 0 THEN
    rc := m2sys_fwrite_bytes(fh, Buffers.SlicePtr(resp^.body), bodyLen)
  END;
  rc := m2sys_fclose(fh);

  (* 4. Cleanup *)
  FreeResponse(resp);

  RETURN HTTPClient.OK
END SyncDownload;

PROCEDURE SyncPut(VAR url: ARRAY OF CHAR;
                  skipVerify: BOOLEAN;
                  bodyData: ADDRESS; bodyLen: INTEGER;
                  VAR contentType: ARRAY OF CHAR;
                  VAR authorization: ARRAY OF CHAR;
                  VAR resp: ResponsePtr): Status;
VAR
  uri: URIRec;
  ust: URI.Status;
  lp: EventLoop.Loop;
  sched: Scheduler;
  est: EventLoop.Status;
  future: Future;
  st: Status;
  settled: BOOLEAN;
  res: Result;
  pst: Promise.Status;
  running: BOOLEAN;
  useH2: BOOLEAN;
BEGIN
  resp := NIL;

  (* 1. Parse URL *)
  ust := URI.Parse(url, uri);
  IF ust # URI.OK THEN RETURN HTTPClient.Invalid END;

  useH2 := IsHTTPS(uri);

  (* 2. Create event loop *)
  est := EventLoop.Create(lp);
  IF est # EventLoop.OK THEN RETURN HTTPClient.ConnectFailed END;

  (* 3. Get scheduler *)
  sched := EventLoop.GetScheduler(lp);

  (* 4. Configure TLS verification *)
  IF skipVerify THEN
    SetSkipVerify(TRUE);
    H2Client.SetSkipVerify(TRUE)
  END;

  (* 5. Issue async PUT — H2 for HTTPS, HTTP/1.1 for HTTP *)
  IF useH2 THEN
    st := H2Client.Put(lp, sched, uri, bodyData, bodyLen,
                       contentType, authorization, future)
  ELSE
    st := HTTPClient.Put(lp, sched, uri, bodyData, bodyLen,
                         contentType, authorization, future)
  END;
  IF st # HTTPClient.OK THEN
    IF skipVerify THEN
      SetSkipVerify(FALSE);
      H2Client.SetSkipVerify(FALSE)
    END;
    est := EventLoop.Destroy(lp);
    RETURN st
  END;

  (* 6. Run event loop until future settles *)
  LOOP
    running := EventLoop.RunOnce(lp);
    pst := GetResultIfSettled(future, settled, res);
    IF settled THEN EXIT END;
    IF NOT running THEN EXIT END
  END;

  (* 7. Extract result *)
  IF settled AND res.isOk THEN
    resp := res.v.ptr
  ELSE
    st := HTTPClient.ConnectFailed
  END;

  (* 8. Cleanup *)
  est := EventLoop.Destroy(lp);
  IF skipVerify THEN
    SetSkipVerify(FALSE);
    H2Client.SetSkipVerify(FALSE)
  END;

  IF resp # NIL THEN
    RETURN HTTPClient.OK
  ELSE
    RETURN st
  END
END SyncPut;

END SyncHTTP.
