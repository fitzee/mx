MODULE http_get;

(* Demonstration of m2http: GET a URL, print status and body.

   Build:
     m2c --m2plus http_get.mod \
       -I ../libs/m2http/src \
       -I ../libs/m2evloop/src \
       -I ../libs/m2futures/src \
       -I ../libs/m2sockets/src \
       ../libs/m2http/src/dns_bridge.c \
       ../libs/m2evloop/src/poller_bridge.c \
       ../libs/m2sockets/src/sockets_bridge.c

   Usage:
     ./http_get                (fetches http://httpbin.org/get)
*)

FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM EventLoop IMPORT Loop, Create, Destroy, GetScheduler, Run;
IMPORT EventLoop;
FROM Scheduler IMPORT Scheduler, TaskProc;
FROM Promise IMPORT Future, GetFate, Fate, GetResultIfSettled, Result;
IMPORT Promise;
FROM URI IMPORT URIRec, Parse;
IMPORT URI;
FROM HTTPClient IMPORT Response, ResponsePtr, Get, FreeResponse, FindHeader;
IMPORT HTTPClient;
IMPORT Buffers;

VAR
  loop: Loop;
  sched: Scheduler;
  est: EventLoop.Status;
  uri: URIRec;
  ust: URI.Status;
  hst: HTTPClient.Status;
  future: Future;
  url: ARRAY [0..255] OF CHAR;

PROCEDURE OnCheck(user: ADDRESS);
VAR
  settled: BOOLEAN;
  res: Result;
  pst: Promise.Status;
  resp: ResponsePtr;
  ct: ARRAY [0..127] OF CHAR;
  bodyLen: INTEGER;
BEGIN
  pst := GetResultIfSettled(future, settled, res);
  IF NOT settled THEN RETURN END;

  IF NOT res.isOk THEN
    WriteString("Request failed with error code: ");
    WriteInt(res.e.code, 0);
    WriteLn;
    EventLoop.Stop(loop);
    RETURN
  END;

  resp := res.v.ptr;
  WriteString("HTTP "); WriteInt(resp^.statusCode, 0); WriteLn;

  (* Print Content-Type header *)
  IF FindHeader(resp, "content-type", ct) THEN
    WriteString("Content-Type: "); WriteString(ct); WriteLn
  END;

  (* Print body length *)
  bodyLen := Buffers.Length(resp^.body);
  WriteString("Body: "); WriteInt(bodyLen, 0);
  WriteString(" bytes"); WriteLn;

  (* Print first 512 bytes of body *)
  IF bodyLen > 0 THEN
    WriteLn;
    WriteString("--- Body (first 512 bytes) ---"); WriteLn;
    PrintBody(resp^.body, bodyLen)
  END;

  FreeResponse(resp);
  EventLoop.Stop(loop)
END OnCheck;

PROCEDURE PrintBody(buf: Buffers.Buffer; len: INTEGER);
VAR i, limit: INTEGER; ch: CHAR; bst: Buffers.Status;
BEGIN
  limit := len;
  IF limit > 512 THEN limit := 512 END;
  FOR i := 0 TO limit - 1 DO
    bst := Buffers.PeekByte(buf, i, ch);
    IF bst = Buffers.OK THEN
      WriteString(ch)
    END
  END;
  WriteLn
END PrintBody;

VAR
  tid: INTEGER;

BEGIN
  url := "http://httpbin.org/get";

  ust := Parse(url, uri);
  IF ust # URI.OK THEN
    WriteString("Bad URL"); WriteLn;
    HALT
  END;

  est := Create(loop);
  sched := GetScheduler(loop);

  hst := Get(loop, sched, uri, future);
  IF hst # HTTPClient.OK THEN
    WriteString("Request failed: ");
    WriteInt(ORD(hst), 0); WriteLn;
    est := Destroy(loop);
    HALT
  END;

  (* Poll periodically to check if the future settled *)
  est := EventLoop.SetInterval(loop, 50, OnCheck, NIL, tid);

  Run(loop);
  est := Destroy(loop)
END http_get.
