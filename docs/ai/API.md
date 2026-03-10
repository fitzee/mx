# mx Library API Quick Reference

Condensed procedure signatures for all 32 libraries. For detailed docs see `docs/libs/<library>/`.

Library name (for `m2.toml` `[deps]`) differs from module name (for `FROM ... IMPORT`).

---

## Core

### m2bytes — ByteBuf, Codec, Hex

```
(* ByteBuf — growable byte buffer *)
PROCEDURE Init(VAR b: Buf; initialCap: CARDINAL);
PROCEDURE Free(VAR b: Buf);
PROCEDURE Clear(VAR b: Buf);
PROCEDURE Reserve(VAR b: Buf; n: CARDINAL);
PROCEDURE AppendByte(VAR b: Buf; val: CARDINAL);
PROCEDURE AppendChars(VAR b: Buf; src: ARRAY OF CHAR; count: CARDINAL);
PROCEDURE AppendView(VAR b: Buf; v: BytesView);
PROCEDURE GetByte(VAR b: Buf; idx: CARDINAL): CARDINAL;
PROCEDURE SetByte(VAR b: Buf; idx: CARDINAL; val: CARDINAL);
PROCEDURE AsView(VAR b: Buf): BytesView;
PROCEDURE Truncate(VAR b: Buf; newLen: CARDINAL);
PROCEDURE DataPtr(VAR b: Buf): ADDRESS;

(* Codec — binary reader/writer with LE/BE/varint *)
PROCEDURE InitReader(VAR r: Reader; data: ADDRESS; len: CARDINAL);
PROCEDURE ReadU8(VAR r: Reader): CARDINAL;
PROCEDURE ReadU16LE(VAR r: Reader): CARDINAL;
PROCEDURE ReadU16BE(VAR r: Reader): CARDINAL;
PROCEDURE ReadU32LE(VAR r: Reader): CARDINAL;
PROCEDURE ReadU32BE(VAR r: Reader): CARDINAL;
PROCEDURE ReadI32LE(VAR r: Reader): INTEGER;
PROCEDURE ReadVarU32(VAR r: Reader): CARDINAL;
PROCEDURE ReadVarI32(VAR r: Reader): INTEGER;
PROCEDURE Skip(VAR r: Reader; n: CARDINAL);
PROCEDURE InitWriter(VAR w: Writer; buf: ADDRESS; cap: CARDINAL);
PROCEDURE WriteU8(VAR w: Writer; val: CARDINAL);
PROCEDURE WriteU16LE(VAR w: Writer; val: CARDINAL);
PROCEDURE WriteU32BE(VAR w: Writer; val: CARDINAL);
PROCEDURE WriteVarU32(VAR w: Writer; val: CARDINAL);
PROCEDURE WriteChars(VAR w: Writer; src: ARRAY OF CHAR; count: CARDINAL);

(* Hex — hex encode/decode *)
PROCEDURE Encode(src: ARRAY OF CHAR; srcLen: CARDINAL; VAR dst: ARRAY OF CHAR): CARDINAL;
PROCEDURE Decode(src: ARRAY OF CHAR; srcLen: CARDINAL; VAR dst: ARRAY OF CHAR): CARDINAL;
PROCEDURE ByteToHex(val: CARDINAL; VAR hi, lo: CHAR);
PROCEDURE HexToByte(hi, lo: CHAR; VAR val: CARDINAL): BOOLEAN;
```

### m2alloc — Arena, Pool, AllocUtil

```
(* Arena — bump allocator over caller-provided buffer *)
PROCEDURE Init(VAR a: Arena; buf: ADDRESS; bufSize: CARDINAL);
PROCEDURE Alloc(VAR a: Arena; size: CARDINAL): ADDRESS;
PROCEDURE Mark(VAR a: Arena): CARDINAL;
PROCEDURE ResetTo(VAR a: Arena; mark: CARDINAL);
PROCEDURE Clear(VAR a: Arena);
PROCEDURE Remaining(VAR a: Arena): CARDINAL;

(* Pool — fixed-size block allocator *)
PROCEDURE Init(VAR p: Pool; buf: ADDRESS; bufSize, blockSize: CARDINAL);
PROCEDURE Alloc(VAR p: Pool): ADDRESS;
PROCEDURE Free(VAR p: Pool; ptr: ADDRESS);
PROCEDURE InUse(VAR p: Pool): CARDINAL;
```

### m2log — Log, LogSinkFile, LogSinkMemory, LogSinkStream

```
(* Log — structured logging *)
PROCEDURE InitDefault();                              (* console sink *)
PROCEDURE Init(VAR log: Logger);
PROCEDURE SetLevel(VAR log: Logger; lvl: Level);      (* TRACE..FATAL *)
PROCEDURE AddSink(VAR log: Logger; s: Sink);
PROCEDURE SetCategory(VAR log: Logger; cat: ARRAY OF CHAR);
PROCEDURE LogMsg(VAR log: Logger; lvl: Level; msg: ARRAY OF CHAR);
PROCEDURE LogKV(VAR log: Logger; lvl: Level; msg: ARRAY OF CHAR; VAR fields: ARRAY OF Field; nFields: INTEGER);
PROCEDURE Info(VAR log: Logger; msg: ARRAY OF CHAR);  (* also Trace/Debug/Warn/Error/Fatal *)
PROCEDURE InfoD(msg: ARRAY OF CHAR);                   (* default logger shortcuts *)
PROCEDURE KVStr(key, val: ARRAY OF CHAR): Field;
PROCEDURE KVInt(key: ARRAY OF CHAR; val: INTEGER): Field;
PROCEDURE KVBool(key: ARRAY OF CHAR; val: BOOLEAN): Field;
PROCEDURE MakeConsoleSink(): Sink;

(* LogSinkFile *)
PROCEDURE Create(path: ARRAY OF CHAR): Sink;
PROCEDURE Close();

(* LogSinkMemory — ring buffer for testing *)
PROCEDURE Create(): Sink;
PROCEDURE GetCount(): CARDINAL;
PROCEDURE GetLine(idx: CARDINAL; VAR buf: ARRAY OF CHAR);
PROCEDURE Contains(substr: ARRAY OF CHAR): BOOLEAN;

(* LogSinkStream — write to TCP/TLS stream *)
PROCEDURE Create(s: Stream): Sink;
```

### m2hash — HashMap

```
PROCEDURE Init(VAR m: Map; VAR buckets: ARRAY OF Bucket);
PROCEDURE Put(VAR m: Map; key: ARRAY OF CHAR; val: INTEGER): BOOLEAN;
PROCEDURE Get(VAR m: Map; key: ARRAY OF CHAR; VAR val: INTEGER): BOOLEAN;
PROCEDURE Contains(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN;
PROCEDURE Remove(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN;
PROCEDURE Count(VAR m: Map): CARDINAL;
```

### m2fsm — Fsm

```
PROCEDURE Init(VAR f: Fsm; nStates, nEvents: CARDINAL; VAR table: ARRAY OF Transition);
PROCEDURE Reset(VAR f: Fsm; initial: StateId);
PROCEDURE Step(VAR f: Fsm; event: EventId): StepStatus;
PROCEDURE CurrentState(VAR f: Fsm): StateId;
PROCEDURE SetActions(VAR f: Fsm; VAR actions: ARRAY OF ActionProc);
PROCEDURE SetGuards(VAR f: Fsm; VAR guards: ARRAY OF GuardProc);
PROCEDURE SetHooks(VAR f: Fsm; entry, exit: HookProc);
PROCEDURE SetTrans(VAR f: Fsm; from: StateId; event: EventId; to: StateId; action: ActionId; guard: GuardId);
```

### m2json — Json

```
(* SAX-style streaming JSON tokenizer — zero allocation *)
(* See docs/libs/m2json/Json.md for full API *)
```

---

## Networking

### m2sockets — Sockets

```
PROCEDURE SocketCreate(family, sockType: INTEGER): Socket;
PROCEDURE CloseSocket(s: Socket): Status;
PROCEDURE Bind(s: Socket; VAR addr: SockAddr): Status;
PROCEDURE Listen(s: Socket; backlog: INTEGER): Status;
PROCEDURE Accept(s: Socket; VAR addr: SockAddr): Socket;
PROCEDURE Connect(s: Socket; VAR addr: SockAddr): Status;
PROCEDURE SendBytes(s: Socket; buf: ADDRESS; len: INTEGER): INTEGER;
PROCEDURE RecvBytes(s: Socket; buf: ADDRESS; maxLen: INTEGER): INTEGER;
PROCEDURE SendString(s: Socket; str: ARRAY OF CHAR): INTEGER;
PROCEDURE RecvLine(s: Socket; VAR buf: ARRAY OF CHAR; maxLen: INTEGER): INTEGER;
PROCEDURE SetNonBlocking(s: Socket; nb: BOOLEAN): Status;
PROCEDURE Shutdown(s: Socket; how: INTEGER): Status;
```

### m2tls — TLS, TlsBridge

Needs `-lssl -lcrypto`.

```
PROCEDURE ContextCreate(): TLSContext;
PROCEDURE ContextDestroy(ctx: TLSContext);
PROCEDURE SetVerifyMode(ctx: TLSContext; mode: VerifyMode);
PROCEDURE LoadSystemRoots(ctx: TLSContext): Status;
PROCEDURE SetALPN(ctx: TLSContext; proto: ARRAY OF CHAR): Status;
PROCEDURE SessionCreate(ctx: TLSContext; fd: INTEGER): TLSSession;
PROCEDURE SessionDestroy(sess: TLSSession);
PROCEDURE SetSNI(sess: TLSSession; hostname: ARRAY OF CHAR): Status;
PROCEDURE Handshake(sess: TLSSession): Status;
PROCEDURE Read(sess: TLSSession; buf: ADDRESS; maxLen: INTEGER): INTEGER;
PROCEDURE Write(sess: TLSSession; buf: ADDRESS; len: INTEGER): INTEGER;
PROCEDURE Shutdown(sess: TLSSession): Status;
PROCEDURE HandshakeAsync(sess: TLSSession): Future;
PROCEDURE ReadAsync(sess: TLSSession; buf: ADDRESS; maxLen: INTEGER): Future;
PROCEDURE WriteAsync(sess: TLSSession; buf: ADDRESS; len: INTEGER): Future;
```

### m2stream — Stream

```
PROCEDURE CreateTCP(fd: INTEGER): Stream;
PROCEDURE CreateTLS(sess: TLSSession; fd: INTEGER): Stream;
PROCEDURE TryRead(s: Stream; buf: ADDRESS; maxLen: INTEGER): INTEGER;
PROCEDURE TryWrite(s: Stream; buf: ADDRESS; len: INTEGER): INTEGER;
PROCEDURE ReadAsync(s: Stream; buf: ADDRESS; maxLen: INTEGER): Future;
PROCEDURE WriteAsync(s: Stream; buf: ADDRESS; len: INTEGER): Future;
PROCEDURE WriteAllAsync(s: Stream; buf: ADDRESS; len: INTEGER): Future;
PROCEDURE CloseAsync(s: Stream): Future;
PROCEDURE GetState(s: Stream): StreamState;
PROCEDURE GetFd(s: Stream): INTEGER;
PROCEDURE Destroy(s: Stream);
```

### m2ws — WebSocket, WsBridge, WsFrame

```
(* WebSocket client — RFC 6455 over m2stream *)
(* See docs/libs/m2ws/WebSocket.md for full API *)
```

---

## HTTP

### m2http — HTTPClient, H2Client, URI, DNS

```
(* HTTPClient *)
PROCEDURE Get(url: ARRAY OF CHAR): ResponsePtr;
PROCEDURE Head(url: ARRAY OF CHAR): ResponsePtr;
PROCEDURE Put(url: ARRAY OF CHAR; body: ADDRESS; bodyLen: INTEGER): ResponsePtr;
PROCEDURE FindHeader(resp: ResponsePtr; name: ARRAY OF CHAR; VAR val: ARRAY OF CHAR): BOOLEAN;
PROCEDURE FreeResponse(resp: ResponsePtr);
PROCEDURE SetSkipVerify(skip: BOOLEAN);

(* H2Client — HTTP/2 over TLS, same interface *)
PROCEDURE Get(url: ARRAY OF CHAR): ResponsePtr;
PROCEDURE Put(url: ARRAY OF CHAR; body: ADDRESS; bodyLen: INTEGER): ResponsePtr;
PROCEDURE FreeResponse(resp: ResponsePtr);

(* URI *)
PROCEDURE Parse(url: ARRAY OF CHAR; VAR uri: URIRec): Status;
PROCEDURE DefaultPort(scheme: ARRAY OF CHAR): INTEGER;
PROCEDURE RequestPath(VAR uri: URIRec; VAR buf: ARRAY OF CHAR);

(* DNS *)
PROCEDURE ResolveA(hostname: ARRAY OF CHAR): Future;
```

### m2http2 — Http2Types, Http2Frame, Http2Hpack, Http2Stream, Http2Conn

```
(* HTTP/2 framing, HPACK header compression, stream/connection FSMs *)
(* See docs/libs/m2http2/ for full API *)
```

### m2http2server — Http2Server, Http2Router, Http2Middleware, ...

```
PROCEDURE Create(VAR opts: ServerOpts): Server;
PROCEDURE AddRoute(srv: Server; method, path: ARRAY OF CHAR; handler: HandlerProc);
PROCEDURE AddMiddleware(srv: Server; mw: MiddlewareProc);
PROCEDURE Start(srv: Server): Status;
PROCEDURE Drain(srv: Server);
PROCEDURE Stop(srv: Server);
PROCEDURE Destroy(srv: Server);
```

---

## Async & Concurrency

### m2evloop — EventLoop, Poller, Timers

```
(* EventLoop — kqueue/epoll event loop *)
PROCEDURE Create(): Loop;
PROCEDURE Destroy(lp: Loop);
PROCEDURE SetTimeout(lp: Loop; ms: CARDINAL; cb: WatcherProc; ctx: ADDRESS): TimerId;
PROCEDURE SetInterval(lp: Loop; ms: CARDINAL; cb: WatcherProc; ctx: ADDRESS): TimerId;
PROCEDURE CancelTimer(lp: Loop; id: TimerId);
PROCEDURE WatchFd(lp: Loop; fd, events: INTEGER; cb: WatcherProc; ctx: ADDRESS): Status;
PROCEDURE UnwatchFd(lp: Loop; fd: INTEGER): Status;
PROCEDURE Enqueue(lp: Loop; task: TaskProc; ctx: ADDRESS);
PROCEDURE Run(lp: Loop);
PROCEDURE Stop(lp: Loop);

(* Poller — low-level kqueue/epoll *)
PROCEDURE Create(): Poller;
PROCEDURE Add(p: Poller; fd, events: INTEGER): Status;
PROCEDURE Wait(p: Poller; VAR buf: EventBuf; timeoutMs: INTEGER): INTEGER;
PROCEDURE NowMs(): CARDINAL;

(* Timers — timer queue *)
PROCEDURE Create(): TimerQueue;
PROCEDURE SetTimeout(tq: TimerQueue; ms: CARDINAL; cb: WatcherProc; ctx: ADDRESS): TimerId;
PROCEDURE Cancel(tq: TimerQueue; id: TimerId);
PROCEDURE Tick(tq: TimerQueue; nowMs: CARDINAL);
```

### m2futures — Promise, Scheduler

```
(* Scheduler *)
PROCEDURE SchedulerCreate(): Scheduler;
PROCEDURE SchedulerDestroy(s: Scheduler);
PROCEDURE SchedulerEnqueue(s: Scheduler; task: TaskProc; ctx: ADDRESS);
PROCEDURE SchedulerPump(s: Scheduler): CARDINAL;

(* Promise/Future *)
PROCEDURE PromiseCreate(s: Scheduler): Promise;
PROCEDURE Resolve(p: Promise; val: Value);
PROCEDURE Reject(p: Promise; err: Error);
PROCEDURE GetFate(f: Future): Fate;            (* Pending | Resolved | Rejected *)
PROCEDURE GetResultIfSettled(f: Future; VAR res: Result): BOOLEAN;
PROCEDURE Map(f: Future; fn: ThenFn; ctx: ADDRESS): Future;
PROCEDURE OnReject(f: Future; fn: CatchFn; ctx: ADDRESS): Future;
PROCEDURE OnSettle(f: Future; fn: VoidFn; ctx: ADDRESS);
PROCEDURE All(s: Scheduler; VAR futures: ARRAY OF Future; count: INTEGER): Future;
PROCEDURE Race(s: Scheduler; VAR futures: ARRAY OF Future; count: INTEGER): Future;
PROCEDURE Ok(s: Scheduler; val: Value): Future;     (* pre-resolved *)
PROCEDURE Fail(s: Scheduler; err: Error): Future;   (* pre-rejected *)
```

### m2pthreads — Threads, ThreadsBridge

```
(* M2+ only — pthreads wrapper *)
(* See docs/libs/m2pthreads/Threads.md for full API *)
```

---

## Services & Security

### m2auth — Auth, AuthBridge, AuthMiddleware

```
PROCEDURE KeyringCreate(): Keyring;
PROCEDURE KeyringAddHS256(kr: Keyring; id: ARRAY OF CHAR; key: ADDRESS; keyLen: CARDINAL): Status;
PROCEDURE KeyringAddEd25519Public(kr: Keyring; id: ARRAY OF CHAR; key: ADDRESS; keyLen: CARDINAL): Status;
PROCEDURE VerifierCreate(kr: Keyring): Verifier;
PROCEDURE VerifyBearerToken(v: Verifier; token: ARRAY OF CHAR; VAR principal: Principal): Status;
PROCEDURE SignToken(kr: Keyring; VAR claims: Claims; VAR buf: ARRAY OF CHAR): Status;
PROCEDURE QuickSignHS256(hexSecret, sub: ARRAY OF CHAR; expSec: INTEGER; VAR buf: ARRAY OF CHAR): BOOLEAN;
PROCEDURE PolicyCreate(): Policy;
PROCEDURE PolicyAllowScope(pol: Policy; scope: ARRAY OF CHAR);
PROCEDURE Authorize(pol: Policy; VAR principal: Principal): Decision;
PROCEDURE ReplayCacheCreate(): ReplayCache;
PROCEDURE ReplayCacheSeenOrAdd(rc: ReplayCache; jti: ARRAY OF CHAR): BOOLEAN;
```

### m2oidc — Oidc, Jwks, OidcBridge

Needs `-lssl -lcrypto`.

```
(* OIDC discovery, JWKS key sets, RS256 JWT verification *)
(* See docs/libs/m2oidc/Oidc.md for full API *)
```

### m2rpc — RpcServer, RpcClient, RpcCodec, RpcFrame, RpcErrors

```
(* RpcServer *)
PROCEDURE InitServer(VAR s: Server; read: ReadFn; write: WriteFn);
PROCEDURE RegisterHandler(VAR s: Server; method: ARRAY OF CHAR; handler: Handler);
PROCEDURE ServeOnce(VAR s: Server): BOOLEAN;
PROCEDURE FreeServer(VAR s: Server);

(* RpcClient *)
PROCEDURE InitClient(VAR c: Client; read: ReadFn; write: WriteFn; sched: Scheduler);
PROCEDURE Call(VAR c: Client; method: ARRAY OF CHAR; body: ADDRESS; bodyLen: CARDINAL): Future;
PROCEDURE OnReadable(VAR c: Client);
PROCEDURE CancelAll(VAR c: Client);
PROCEDURE FreeClient(VAR c: Client);
```

---

## Database

### m2sqlite — SQLite, SQLiteBridge

Needs `-lsqlite3`.

```
(* See docs/libs/m2sqlite/SQLite.md for full API *)
```

### m2lmdb — Lmdb, LmdbBridge

Needs `-llmdb`.

```
(* See docs/libs/m2lmdb/Lmdb.md for full API *)
```

---

## Data Formats

### m2conf — Conf

```
PROCEDURE Parse(text: ARRAY OF CHAR; textLen: CARDINAL): BOOLEAN;
PROCEDURE Clear();
PROCEDURE SectionCount(): INTEGER;
PROCEDURE GetSectionName(idx: INTEGER; VAR name: ARRAY OF CHAR);
PROCEDURE KeyCount(section: INTEGER): INTEGER;
PROCEDURE GetKey(section, idx: INTEGER; VAR key: ARRAY OF CHAR);
PROCEDURE GetValue(section: INTEGER; key: ARRAY OF CHAR; VAR val: ARRAY OF CHAR): BOOLEAN;
PROCEDURE HasKey(section: INTEGER; key: ARRAY OF CHAR): BOOLEAN;
```

### m2fmt — Fmt

```
PROCEDURE InitBuf(VAR b: Buf);
PROCEDURE JsonStart(VAR b: Buf);
PROCEDURE JsonEnd(VAR b: Buf);
PROCEDURE JsonKey(VAR b: Buf; key: ARRAY OF CHAR);
PROCEDURE JsonStr(VAR b: Buf; val: ARRAY OF CHAR);
PROCEDURE JsonInt(VAR b: Buf; val: INTEGER);
PROCEDURE JsonBool(VAR b: Buf; val: BOOLEAN);
PROCEDURE CsvField(VAR b: Buf; val: ARRAY OF CHAR);
PROCEDURE CsvSep(VAR b: Buf);
PROCEDURE CsvNewline(VAR b: Buf);
PROCEDURE TableSetColumns(n: INTEGER);
PROCEDURE TableRender(VAR b: Buf);
```

---

## Utilities

### m2cli — CLI

```
PROCEDURE AddFlag(short, long, help: ARRAY OF CHAR);
PROCEDURE AddOption(short, long, help, default: ARRAY OF CHAR);
PROCEDURE Parse(getArg: GetArgProc; argc: INTEGER);
PROCEDURE HasFlag(name: ARRAY OF CHAR): BOOLEAN;
PROCEDURE GetOption(name: ARRAY OF CHAR; VAR val: ARRAY OF CHAR): BOOLEAN;
PROCEDURE PrintHelp();
```

### m2path — Path

```
PROCEDURE Normalize(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
PROCEDURE Extension(path: ARRAY OF CHAR; VAR ext: ARRAY OF CHAR);
PROCEDURE StripExt(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
PROCEDURE IsAbsolute(path: ARRAY OF CHAR): BOOLEAN;
PROCEDURE Split(path: ARRAY OF CHAR; VAR dir, base: ARRAY OF CHAR);
PROCEDURE RelativeTo(path, base: ARRAY OF CHAR; VAR out: ARRAY OF CHAR): BOOLEAN;
PROCEDURE Join(a, b: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
PROCEDURE Match(pattern, basename: ARRAY OF CHAR): BOOLEAN;
```

### m2glob — Glob

```
PROCEDURE Match(pattern, text: ARRAY OF CHAR): BOOLEAN;
PROCEDURE IsNegated(pattern: ARRAY OF CHAR): BOOLEAN;
PROCEDURE StripNegation(pattern: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
```

### m2regex — Regex, RegexBridge

Uses POSIX `regex.h` (no external library needed).

```
(* See docs/libs/m2regex/Regex.md for full API *)
```

### m2text — Text

```
PROCEDURE IsValidUTF8(buf: ADDRESS; len: CARDINAL): BOOLEAN;
PROCEDURE IsASCII(buf: ADDRESS; len: CARDINAL): BOOLEAN;
PROCEDURE IsText(buf: ADDRESS; len: CARDINAL): BOOLEAN;
PROCEDURE IsBinary(buf: ADDRESS; len: CARDINAL): BOOLEAN;
PROCEDURE HasBOM(buf: ADDRESS; len: CARDINAL): BOOLEAN;
PROCEDURE CountLines(buf: ADDRESS; len: CARDINAL): CARDINAL;
PROCEDURE DetectLineEnding(buf: ADDRESS; len: CARDINAL): INTEGER;
```

### m2tok — Tokenizer

```
PROCEDURE Init(VAR s: State; buf: ADDRESS; len: CARDINAL);
PROCEDURE Next(VAR s: State; VAR tok: Token): BOOLEAN;
PROCEDURE CopyToken(VAR tok: Token; VAR buf: ARRAY OF CHAR);
```

### m2zlib — Zlib, ZlibBridge

Needs `-lz`.

```
(* See docs/libs/m2zlib/Zlib.md for full API *)
```

### m2sys — Sys (C FFI shim)

Consumed via `DEFINITION MODULE FOR "C"`. All functions take `ADDRESS` for strings (use `ADR("...")`).

```
(* File I/O *)
m2sys_fopen(path, mode: ADDRESS): INTEGER
m2sys_fclose(handle: INTEGER): INTEGER
m2sys_fread_line(handle: INTEGER; buf: ADDRESS; bufSize: INTEGER): INTEGER
m2sys_fwrite_str(handle: INTEGER; data: ADDRESS): INTEGER
m2sys_fwrite_bytes(handle: INTEGER; data: ADDRESS; len: INTEGER): INTEGER
m2sys_fread_bytes(handle: INTEGER; buf: ADDRESS; maxLen: INTEGER): INTEGER
m2sys_fseek(handle: INTEGER; offset: LONGINT; whence: INTEGER): INTEGER
m2sys_file_exists(path: ADDRESS): INTEGER       (* 1=yes, 0=no *)
m2sys_is_dir(path: ADDRESS): INTEGER
m2sys_file_size(path: ADDRESS): LONGINT
m2sys_file_mtime(path: ADDRESS): LONGINT

(* Directory operations *)
m2sys_mkdir_p(path: ADDRESS): INTEGER
m2sys_remove_file(path: ADDRESS): INTEGER
m2sys_copy_file(src, dst: ADDRESS): INTEGER
m2sys_rename(oldPath, newPath: ADDRESS): INTEGER
m2sys_rmdir_r(path: ADDRESS): INTEGER
m2sys_list_dir(dir, buf: ADDRESS; bufSize: INTEGER): INTEGER

(* Process execution *)
m2sys_exec(cmdline: ADDRESS): INTEGER
m2sys_exec_output(cmdline, outBuf: ADDRESS; outSize: INTEGER): INTEGER
m2sys_exit(code: INTEGER)

(* Crypto *)
m2sys_sha256_str(data: ADDRESS; len: INTEGER; hexOut: ADDRESS)
m2sys_sha256_file(path, hexOut: ADDRESS): INTEGER

(* Path utilities *)
m2sys_join_path(a, b, out: ADDRESS; outSize: INTEGER)
m2sys_home_dir(out: ADDRESS; outSize: INTEGER)
m2sys_getcwd(out: ADDRESS; outSize: INTEGER)
m2sys_getenv(name, out: ADDRESS; outSize: INTEGER)
m2sys_basename(path, out: ADDRESS; outSize: INTEGER)
m2sys_dirname(path, out: ADDRESS; outSize: INTEGER)

(* Archive *)
m2sys_tar_create(archivePath, baseDir: ADDRESS): INTEGER
m2sys_tar_extract(archivePath, destDir: ADDRESS): INTEGER

(* Time *)
m2sys_unix_time(): LONGINT
```

---

## Graphics

### m2gfx — Gfx, Canvas, Font, Events, Texture, PixBuf, Color, DrawAlgo

Needs `-lSDL2 -lSDL2_ttf`.

```
(* Gfx — window + renderer management *)
PROCEDURE Init(): BOOLEAN;
PROCEDURE InitFont(): BOOLEAN;
PROCEDURE Quit();
PROCEDURE CreateWindow(title: ARRAY OF CHAR; x, y, w, h, flags: INTEGER): Window;
PROCEDURE DestroyWindow(win: Window);
PROCEDURE CreateRenderer(win: Window; flags: INTEGER): Renderer;
PROCEDURE Present(ren: Renderer);
PROCEDURE Delay(ms: CARDINAL);

(* Canvas — 2D drawing *)
PROCEDURE SetColor(ren: Renderer; r, g, b, a: INTEGER);
PROCEDURE Clear(ren: Renderer);
PROCEDURE DrawRect(ren: Renderer; x, y, w, h: INTEGER);
PROCEDURE FillRect(ren: Renderer; x, y, w, h: INTEGER);
PROCEDURE DrawLine(ren: Renderer; x1, y1, x2, y2: INTEGER);
PROCEDURE DrawCircle(ren: Renderer; cx, cy, radius: INTEGER);
PROCEDURE FillCircle(ren: Renderer; cx, cy, radius: INTEGER);
PROCEDURE DrawEllipse(ren: Renderer; cx, cy, rx, ry: INTEGER);
PROCEDURE FillTriangle(ren: Renderer; x1, y1, x2, y2, x3, y3: INTEGER);

(* Font — TTF text rendering *)
PROCEDURE Open(path: ARRAY OF CHAR; ptSize: INTEGER): FontHandle;
PROCEDURE Close(font: FontHandle);
PROCEDURE DrawText(ren: Renderer; font: FontHandle; text: ARRAY OF CHAR; x, y, r, g, b, a: INTEGER);
PROCEDURE DrawTextWrapped(ren: Renderer; font: FontHandle; text: ARRAY OF CHAR; x, y, wrapWidth, r, g, b, a: INTEGER);
PROCEDURE TextWidth(font: FontHandle; text: ARRAY OF CHAR): INTEGER;
PROCEDURE Height(font: FontHandle): INTEGER;

(* Events — SDL event polling *)
PROCEDURE Poll(): INTEGER;           (* returns event type or 0 *)
PROCEDURE Wait(): INTEGER;
PROCEDURE KeyCode(): INTEGER;
PROCEDURE KeyMod(): INTEGER;
PROCEDURE MouseX(): INTEGER;
PROCEDURE MouseY(): INTEGER;
PROCEDURE MouseButton(): INTEGER;
PROCEDURE WheelY(): INTEGER;
PROCEDURE TextInput(VAR buf: ARRAY OF CHAR);
PROCEDURE IsKeyPressed(scancode: INTEGER): BOOLEAN;

(* Texture *)
PROCEDURE LoadBMP(ren: Renderer; path: ARRAY OF CHAR): Tex;
PROCEDURE Create(ren: Renderer; w, h: INTEGER): Tex;
PROCEDURE FromText(ren: Renderer; font: FontHandle; text: ARRAY OF CHAR; r, g, b, a: INTEGER): Tex;
PROCEDURE Destroy(tex: Tex);
PROCEDURE Draw(ren: Renderer; tex: Tex; x, y: INTEGER);
PROCEDURE DrawRegion(ren: Renderer; tex: Tex; sx, sy, sw, sh, dx, dy, dw, dh: INTEGER);

(* Color *)
PROCEDURE Pack(r, g, b: INTEGER): CARDINAL;
PROCEDURE Blend(c1, c2: CARDINAL; t: INTEGER): CARDINAL;

(* PixBuf — 8-bit indexed pixel buffer, 150+ procedures *)
(* See libs/m2gfx/src/PixBuf.def for full API *)
```
