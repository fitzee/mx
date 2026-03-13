# M2 Standard Library API Reference

| Library | Module | Procedure | Description |
|---------|--------|-----------|-------------|
| m2alloc | AllocUtil | AlignUp | Round x up to the next multiple of align (must be power of two) |
| m2alloc | AllocUtil | FillBytes | Fill count bytes starting at base with val (0..255) |
| m2alloc | AllocUtil | IsPowerOfTwo | Returns TRUE if x is a power of two (x > 0) |
| m2alloc | AllocUtil | PtrAdd | Return base + offset as an ADDRESS |
| m2alloc | AllocUtil | PtrDiff | Return a - b as a CARDINAL; returns 0 if b >= a |
| m2alloc | AllocUtil | ReadAddr | Read an ADDRESS from the memory location loc |
| m2alloc | AllocUtil | WriteAddr | Write val as an ADDRESS at the memory location loc |
| m2alloc | Arena | Alloc | Allocate n bytes with the given alignment; sets p and ok |
| m2alloc | Arena | Clear | Reset arena position to 0 |
| m2alloc | Arena | FailedAllocs | Return number of failed Alloc calls |
| m2alloc | Arena | HighWater | Return peak allocation position ever reached |
| m2alloc | Arena | Init | Initialise an arena over the backing store [base..base+size) |
| m2alloc | Arena | Mark | Return the current position for later ResetTo |
| m2alloc | Arena | PoisonOff | Disable poison |
| m2alloc | Arena | PoisonOn | Enable poison: alloc fills 0CDH, reset fills 0 |
| m2alloc | Arena | Remaining | Return bytes remaining in the arena |
| m2alloc | Arena | ResetTo | Reset pos to mark; if poison is on, freed bytes are filled with 0 |
| m2alloc | Pool | Alloc | Allocate one block from the pool; sets out and ok |
| m2alloc | Pool | Free | Return a block to the pool with validation; sets ok |
| m2alloc | Pool | HighWater | Return peak number of blocks ever allocated simultaneously |
| m2alloc | Pool | Init | Initialise a pool over the backing store with given block size |
| m2alloc | Pool | InUse | Return number of blocks currently allocated |
| m2alloc | Pool | InvalidFrees | Return number of invalid Free calls |
| m2alloc | Pool | PoisonOff | Disable poison |
| m2alloc | Pool | PoisonOn | Enable poison: alloc fills 0CDH, free fills 0DDH |
| m2auth | Auth | Authorize | Authorize a principal against the policy; returns OK or Denied |
| m2auth | Auth | DecodeHexKey | Decode a 64-char hex string into a 32-byte SymKey |
| m2auth | Auth | InitPrincipal | Initialize a Principal record to default values |
| m2auth | Auth | KeyringAddEd25519Public | Add an Ed25519 public key to the keyring |
| m2auth | Auth | KeyringAddHS256 | Add an HS256 symmetric key to the keyring |
| m2auth | Auth | KeyringCreate | Create a new keyring handle |
| m2auth | Auth | KeyringDestroy | Destroy a keyring and free its resources |
| m2auth | Auth | KeyringList | List all key IDs in the keyring |
| m2auth | Auth | KeyringRemove | Remove a key from the keyring by key ID |
| m2auth | Auth | KeyringSetActive | Set the active signing key by key ID |
| m2auth | Auth | PolicyAllowClaimEquals | Allow access when a claim key equals a specific value |
| m2auth | Auth | PolicyAllowScope | Allow access for a specific scope in the policy |
| m2auth | Auth | PolicyCreate | Create a new authorization policy handle |
| m2auth | Auth | PolicyDestroy | Destroy a policy and free its resources |
| m2auth | Auth | QuickSignHS256 | One-shot: decode hex secret, sign a short-lived JWT HS256 token |
| m2auth | Auth | ReplayCacheCreate | Create a new replay detection cache |
| m2auth | Auth | ReplayCacheDestroy | Destroy a replay cache and free its resources |
| m2auth | Auth | ReplayCacheSeenOrAdd | Check if jti is new or already seen; adds if new |
| m2auth | Auth | SignToken | Sign a principal into a token string with the given algorithm |
| m2auth | Auth | StatusToStr | Convert a Status value to its string representation |
| m2auth | Auth | VerifierCreate | Create a new token verifier with the given keyring |
| m2auth | Auth | VerifierDestroy | Destroy a verifier and free its resources |
| m2auth | Auth | VerifierSetAudience | Set the expected audience for token verification |
| m2auth | Auth | VerifierSetClockSkewSeconds | Set the allowed clock skew in seconds for verification |
| m2auth | Auth | VerifierSetIssuer | Set the expected issuer for token verification |
| m2auth | Auth | VerifyBearerToken | Verify a bearer token and populate the principal on success |
| m2auth | AuthBridge | m2_auth_b64url_decode | Base64url decode src into dst; returns bytes written or -1 |
| m2auth | AuthBridge | m2_auth_b64url_encode | Base64url encode src into dst; returns bytes written or -1 |
| m2auth | AuthBridge | m2_auth_ct_compare | Constant-time compare two buffers; returns 0 if equal |
| m2auth | AuthBridge | m2_auth_ed25519_keygen | Generate Ed25519 key pair; returns 0 on success |
| m2auth | AuthBridge | m2_auth_ed25519_sign | Sign a message with Ed25519 private key; returns 0 on success |
| m2auth | AuthBridge | m2_auth_ed25519_verify | Verify Ed25519 signature; returns 0 if valid |
| m2auth | AuthBridge | m2_auth_get_unix_time | Return current Unix timestamp in seconds since epoch |
| m2auth | AuthBridge | m2_auth_has_ed25519 | Returns 1 if Ed25519 is available, 0 otherwise |
| m2auth | AuthBridge | m2_auth_hmac_sha256 | Compute HMAC-SHA256; returns 0 on success |
| m2auth | AuthBridge | m2_auth_init | Initialise the auth backend (idempotent) |
| m2auth | AuthMiddleware | AuthMw | HTTP/2 middleware that verifies Bearer token and authorizes |
| m2auth | AuthMiddleware | Configure | Configure the middleware's verifier and policy |
| m2auth | AuthMiddleware | GetPrincipal | Retrieve the principal from the last successful AuthMw call |
| m2auth | AuthMiddleware | RpcAuthGuard | RPC auth guard that extracts and verifies token from request body |
| m2auth | AuthMiddleware | SetAuditCallback | Set audit callback procedure with context pointer |
| m2auth | AuthMiddleware | SetRpcGuardHandler | Set the inner handler that RpcAuthGuard delegates to |
| m2bytes | ByteBuf | AppendByte | Append a single byte (0..255) to the buffer |
| m2bytes | ByteBuf | AppendChars | Append bytes from an open CHAR array to the buffer |
| m2bytes | ByteBuf | AppendView | Append contents of a BytesView to the buffer |
| m2bytes | ByteBuf | AsView | Return a zero-copy view of the buffer's current contents |
| m2bytes | ByteBuf | Clear | Reset buffer length to 0, keep allocated capacity |
| m2bytes | ByteBuf | Free | Free the buffer's backing memory; sets len/cap to 0 |
| m2bytes | ByteBuf | GetByte | Get byte at index as 0..255; returns 0 if out of bounds |
| m2bytes | ByteBuf | Init | Allocate a buffer with the given initial capacity |
| m2bytes | ByteBuf | Reserve | Ensure at least len+extra bytes of capacity; grows geometrically |
| m2bytes | ByteBuf | SetByte | Set byte at index; no-op if out of bounds |
| m2bytes | ByteBuf | Truncate | Truncate the buffer to newLen; no-op if newLen >= len |
| m2bytes | ByteBuf | ViewGetByte | Get byte at index from a BytesView; returns 0 if out of bounds |
| m2bytes | Codec | InitReader | Initialize a binary reader from a BytesView |
| m2bytes | Codec | InitWriter | Initialize a binary writer targeting a Buf |
| m2bytes | Codec | ReadI32BE | Read 32-bit signed integer, big-endian |
| m2bytes | Codec | ReadI32LE | Read 32-bit signed integer, little-endian |
| m2bytes | Codec | ReadSlice | Read n bytes as a zero-copy sub-view |
| m2bytes | Codec | ReadU16BE | Read 16-bit unsigned integer, big-endian |
| m2bytes | Codec | ReadU16LE | Read 16-bit unsigned integer, little-endian |
| m2bytes | Codec | ReadU32BE | Read 32-bit unsigned integer, big-endian |
| m2bytes | Codec | ReadU32LE | Read 32-bit unsigned integer, little-endian |
| m2bytes | Codec | ReadU8 | Read a single unsigned byte (0..255) |
| m2bytes | Codec | ReadVarI32 | Decode signed 32-bit ZigZag varint |
| m2bytes | Codec | ReadVarU32 | Decode unsigned 32-bit LEB128 varint |
| m2bytes | Codec | Remaining | Return bytes remaining from current reader position |
| m2bytes | Codec | Skip | Skip n bytes in the reader; sets ok=FALSE if insufficient |
| m2bytes | Codec | WriteChars | Write bytes from an open CHAR array |
| m2bytes | Codec | WriteI32BE | Write 32-bit signed integer, big-endian |
| m2bytes | Codec | WriteI32LE | Write 32-bit signed integer, little-endian |
| m2bytes | Codec | WriteU16BE | Write 16-bit unsigned integer, big-endian |
| m2bytes | Codec | WriteU16LE | Write 16-bit unsigned integer, little-endian |
| m2bytes | Codec | WriteU32BE | Write 32-bit unsigned integer, big-endian |
| m2bytes | Codec | WriteU32LE | Write 32-bit unsigned integer, little-endian |
| m2bytes | Codec | WriteU8 | Write a single unsigned byte (0..255) |
| m2bytes | Codec | WriteVarI32 | Encode signed 32-bit as ZigZag + LEB128 |
| m2bytes | Codec | WriteVarU32 | Encode unsigned 32-bit as LEB128 varint |
| m2bytes | Hex | ByteToHex | Encode a single byte (0..255) into 2 hex characters |
| m2bytes | Hex | Decode | Decode hex string into a Buf (appends to existing contents) |
| m2bytes | Hex | Encode | Encode bytes from a Buf into hex characters |
| m2bytes | Hex | HexToByte | Decode 2 hex characters to a byte value (0..255) |
| m2cli | CLI | AddFlag | Add a boolean flag with short and long forms |
| m2cli | CLI | AddOption | Add an option that takes a value with short and long forms |
| m2cli | CLI | GetOption | Return 1 if option was present and copy value to buf; 0 otherwise |
| m2cli | CLI | HasFlag | Return 1 if the flag was present, 0 otherwise |
| m2cli | CLI | Parse | Parse command-line arguments using argc and getArg procedure |
| m2cli | CLI | PrintHelp | Print usage help based on registered flags and options |
| m2conf | Conf | Clear | Reset all parsed configuration data |
| m2conf | Conf | GetKey | Get key name by index within a section |
| m2conf | Conf | GetSectionName | Get section name by index; returns TRUE if valid |
| m2conf | Conf | GetValue | Get value for a section+key pair; returns TRUE if found |
| m2conf | Conf | HasKey | Check if a key exists in a given section |
| m2conf | Conf | KeyCount | Return number of keys in the named section; -1 if not found |
| m2conf | Conf | Parse | Parse INI-format config from buffer; returns TRUE on success |
| m2conf | Conf | SectionCount | Return number of sections parsed |
| m2evloop | EventLoop | CancelTimer | Cancel a timer by id (idempotent) |
| m2evloop | EventLoop | Create | Create an event loop with Poller, TimerQueue, and Scheduler |
| m2evloop | EventLoop | Destroy | Destroy the event loop and all owned resources |
| m2evloop | EventLoop | Enqueue | Enqueue a microtask for execution during the next pump |
| m2evloop | EventLoop | GetScheduler | Return the underlying Scheduler handle |
| m2evloop | EventLoop | ModifyFd | Change the interest set for an already-watched fd |
| m2evloop | EventLoop | Run | Run the event loop until Stop is called or all work completes |
| m2evloop | EventLoop | RunOnce | Execute one iteration of the event loop; returns TRUE if work remains |
| m2evloop | EventLoop | SetInterval | Schedule a repeating timer with the given interval |
| m2evloop | EventLoop | SetTimeout | Schedule a one-shot timer with the given delay |
| m2evloop | EventLoop | Stop | Signal the event loop to stop after the current iteration |
| m2evloop | EventLoop | UnwatchFd | Stop watching a file descriptor |
| m2evloop | EventLoop | WatchFd | Watch a file descriptor for readiness events |
| m2evloop | Poller | Add | Register fd for the given event interest set |
| m2evloop | Poller | Create | Create a new poller instance |
| m2evloop | Poller | Destroy | Destroy a poller instance and release resources |
| m2evloop | Poller | Modify | Modify interest set for an already-registered fd |
| m2evloop | Poller | NowMs | Return current monotonic time in milliseconds |
| m2evloop | Poller | Remove | Remove fd from the poller |
| m2evloop | Poller | Wait | Wait up to timeoutMs milliseconds for I/O events |
| m2evloop | PollerBridge | m2_now_ms | Return current monotonic time in milliseconds |
| m2evloop | PollerBridge | m2_poller_add | Register fd with events on a poller handle |
| m2evloop | PollerBridge | m2_poller_create | Create a new OS-level poller handle |
| m2evloop | PollerBridge | m2_poller_del | Remove fd from a poller handle |
| m2evloop | PollerBridge | m2_poller_destroy | Destroy an OS-level poller handle |
| m2evloop | PollerBridge | m2_poller_mod | Modify fd event interest on a poller handle |
| m2evloop | PollerBridge | m2_poller_wait | Poll for events with timeout; returns ready events |
| m2evloop | Timers | ActiveCount | Return the number of active timers |
| m2evloop | Timers | Cancel | Cancel a pending timer by id (idempotent) |
| m2evloop | Timers | Create | Create a timer queue with the given scheduler |
| m2evloop | Timers | Destroy | Destroy a timer queue and free resources |
| m2evloop | Timers | NextDeadline | Return ms until next timer fires, or -1 if no timers |
| m2evloop | Timers | SetInterval | Schedule a repeating timer at the given interval |
| m2evloop | Timers | SetTimeout | Schedule a one-shot timer with the given delay |
| m2evloop | Timers | Tick | Fire all timers whose deadline <= now; enqueue callbacks |
| m2fmt | Fmt | BufClear | Reset buffer position to 0 |
| m2fmt | Fmt | BufLen | Return current number of bytes written to buffer |
| m2fmt | Fmt | CsvField | Write a CSV field with auto-quoting per RFC 4180 |
| m2fmt | Fmt | CsvNewline | Write CSV line ending (CRLF) |
| m2fmt | Fmt | CsvSep | Write CSV field separator (comma) |
| m2fmt | Fmt | InitBuf | Initialise buffer with caller-provided backing array |
| m2fmt | Fmt | JsonArrayEnd | End JSON array: write ']' |
| m2fmt | Fmt | JsonArrayStart | Begin JSON array: write '[' |
| m2fmt | Fmt | JsonBool | Write a JSON boolean value with comma handling |
| m2fmt | Fmt | JsonEnd | End JSON object: write '}' |
| m2fmt | Fmt | JsonInt | Write a JSON integer value with comma handling |
| m2fmt | Fmt | JsonKey | Write a JSON key with comma handling |
| m2fmt | Fmt | JsonNull | Write a JSON null value with comma handling |
| m2fmt | Fmt | JsonStart | Begin JSON object: write '{' |
| m2fmt | Fmt | JsonStr | Write a JSON string value with escaping and comma handling |
| m2fmt | Fmt | TableAddRow | Add a row to the text table; returns row index |
| m2fmt | Fmt | TableRender | Render the text table into the buffer |
| m2fmt | Fmt | TableSetCell | Set cell value at given row and column |
| m2fmt | Fmt | TableSetColumns | Set number of columns for the text table (max 16) |
| m2fmt | Fmt | TableSetHeader | Set header text for a column |
| m2fsm | Fsm | ClearTable | Fill n Transition entries with NoState/NoAction/NoGuard |
| m2fsm | Fsm | CurrentState | Return current state of the FSM |
| m2fsm | Fsm | ErrorCount | Return number of action errors |
| m2fsm | Fsm | Init | Initialise an FSM with a dense transition table |
| m2fsm | Fsm | InvalidCount | Return number of events with no matching transition |
| m2fsm | Fsm | RejectCount | Return number of transitions rejected by guards |
| m2fsm | Fsm | Reset | Reset state to start and clear all counters |
| m2fsm | Fsm | SetActions | Set the action callback array for the FSM |
| m2fsm | Fsm | SetGuards | Set the guard callback array for the FSM |
| m2fsm | Fsm | SetHooks | Set state entry/exit hook arrays |
| m2fsm | Fsm | SetTrace | Set the trace callback for debugging |
| m2fsm | Fsm | SetTrans | Fill a Transition record with next state, action, and guard |
| m2fsm | Fsm | Step | Process one event through the FSM |
| m2fsm | Fsm | StepCount | Return number of successful transitions |
| m2fsm | FsmTrace | ConsoleTrace | Print a human-readable trace line to stdout |
| m2futures | Future | FAll | Combine futures: fulfills when all fulfill, rejects on first rejection |
| m2futures | Future | FMap | Attach a transformation to a future |
| m2futures | Future | FOnReject | Attach an error handler to a future |
| m2futures | Future | FOnSettle | Attach a side-effect observer to a future |
| m2futures | Future | FRace | Combine futures: settles as soon as the first future settles |
| m2futures | Future | GetFate | Query the current fate (Pending/Fulfilled/Rejected) of a future |
| m2futures | Future | GetResultIfSettled | Query the result if the future is settled |
| m2futures | Promise | All | Join futures: fulfills when every future fulfills |
| m2futures | Promise | Fail | Create a Result with failure from an Error |
| m2futures | Promise | GetFate | Query the current fate of a future |
| m2futures | Promise | GetResultIfSettled | Query the result if the future is settled |
| m2futures | Promise | MakeError | Construct an Error value from code and pointer |
| m2futures | Promise | MakeValue | Construct a Value from tag and pointer |
| m2futures | Promise | Map | Attach a transformation to a future |
| m2futures | Promise | Ok | Create a Result with success from a Value |
| m2futures | Promise | OnReject | Attach an error handler called only on rejection |
| m2futures | Promise | OnSettle | Attach a side-effect observer called on settlement |
| m2futures | Promise | PromiseCreate | Create a linked promise/future pair on a scheduler |
| m2futures | Promise | Race | Settle as soon as the first future in the array settles |
| m2futures | Promise | Reject | Reject the promise with an error |
| m2futures | Promise | Resolve | Fulfill the promise with a value |
| m2futures | Scheduler | SchedulerCreate | Create a scheduler with room for up to capacity queued tasks |
| m2futures | Scheduler | SchedulerDestroy | Destroy a scheduler and free its resources |
| m2futures | Scheduler | SchedulerEnqueue | Enqueue a callback for execution on the next pump cycle |
| m2futures | Scheduler | SchedulerPump | Run up to maxSteps queued callbacks |
| m2gfx | Canvas | Clear | Clear the renderer |
| m2gfx | Canvas | ClearClip | Clear the clipping rectangle |
| m2gfx | Canvas | DrawArc | Draw an arc with center, radius, and angle range |
| m2gfx | Canvas | DrawBezier | Draw a cubic Bezier curve with control points |
| m2gfx | Canvas | DrawCircle | Draw a circle outline |
| m2gfx | Canvas | DrawEllipse | Draw an ellipse outline |
| m2gfx | Canvas | DrawLine | Draw a line between two points |
| m2gfx | Canvas | DrawPoint | Draw a single point |
| m2gfx | Canvas | DrawRect | Draw a rectangle outline |
| m2gfx | Canvas | DrawRoundRect | Draw a rounded rectangle outline |
| m2gfx | Canvas | DrawThickLine | Draw a line with specified thickness |
| m2gfx | Canvas | DrawTriangle | Draw a triangle outline |
| m2gfx | Canvas | FillCircle | Draw a filled circle |
| m2gfx | Canvas | FillEllipse | Draw a filled ellipse |
| m2gfx | Canvas | FillRect | Draw a filled rectangle |
| m2gfx | Canvas | FillRoundRect | Draw a filled rounded rectangle |
| m2gfx | Canvas | FillTriangle | Draw a filled triangle |
| m2gfx | Canvas | GetClipH | Get clipping rectangle height |
| m2gfx | Canvas | GetClipW | Get clipping rectangle width |
| m2gfx | Canvas | GetClipX | Get clipping rectangle X origin |
| m2gfx | Canvas | GetClipY | Get clipping rectangle Y origin |
| m2gfx | Canvas | GetColor | Get current draw color RGBA components |
| m2gfx | Canvas | ResetViewport | Reset viewport to full renderer area |
| m2gfx | Canvas | SetBlendMode | Set the blend mode for rendering |
| m2gfx | Canvas | SetClip | Set the clipping rectangle |
| m2gfx | Canvas | SetColor | Set the draw color RGBA components |
| m2gfx | Canvas | SetViewport | Set the viewport for rendering |
| m2gfx | Color | Blend | Integer percentage interpolation between base and target |
| m2gfx | Color | Pack | Pack RGB into RGBA8888 with A=0xFF |
| m2gfx | Color | PackAlpha | Pack RGBA into RGBA8888 |
| m2gfx | Color | UnpackB | Extract blue channel (bits 15..8) from packed color |
| m2gfx | Color | UnpackG | Extract green channel (bits 23..16) from packed color |
| m2gfx | Color | UnpackR | Extract red channel (bits 31..24) from packed color |
| m2gfx | DrawAlgo | Bezier | Draw a cubic Bezier curve via line callback |
| m2gfx | DrawAlgo | Circle | Draw a midpoint circle via point callback |
| m2gfx | DrawAlgo | Ellipse | Draw a midpoint ellipse via point callback |
| m2gfx | DrawAlgo | FillCircle | Draw a filled circle via horizontal line callback |
| m2gfx | DrawAlgo | FillEllipse | Draw a filled ellipse via horizontal line callback |
| m2gfx | DrawAlgo | FillTriangle | Draw a filled triangle via horizontal line callback |
| m2gfx | DrawAlgo | Line | Draw a Bresenham line via point callback |
| m2gfx | DrawAlgo | Triangle | Draw a triangle outline via line callback |
| m2gfx | Events | GetMouseGlobal | Get global mouse position and button state |
| m2gfx | Events | GetMouseState | Get mouse position relative to window and button state |
| m2gfx | Events | IsKeyPressed | Check if a key is currently pressed by scancode |
| m2gfx | Events | IsTextInputActive | Check if text input mode is active |
| m2gfx | Events | KeyCode | Get key code from last key event |
| m2gfx | Events | KeyMod | Get modifier flags from last key event |
| m2gfx | Events | MouseButton | Get mouse button from last mouse event |
| m2gfx | Events | MouseX | Get mouse X coordinate from last mouse event |
| m2gfx | Events | MouseY | Get mouse Y coordinate from last mouse event |
| m2gfx | Events | Poll | Poll for pending events; returns event type |
| m2gfx | Events | ScanCode | Get scan code from last key event |
| m2gfx | Events | StartTextInput | Enable text input mode |
| m2gfx | Events | StopTextInput | Disable text input mode |
| m2gfx | Events | TextInput | Get text input string from last text input event |
| m2gfx | Events | TextInputLen | Get length of text input from last text input event |
| m2gfx | Events | Wait | Wait for an event; returns event type |
| m2gfx | Events | WaitTimeout | Wait up to ms milliseconds for an event |
| m2gfx | Events | WarpMouse | Warp mouse cursor to position in window |
| m2gfx | Events | WheelX | Get horizontal scroll amount from last wheel event |
| m2gfx | Events | WheelY | Get vertical scroll amount from last wheel event |
| m2gfx | Events | WindowEvent | Get window event subtype from last window event |
| m2gfx | Events | WindowID | Get window ID from last window event |
| m2gfx | Font | Ascent | Get font ascent in pixels |
| m2gfx | Font | Close | Close a font handle and free resources |
| m2gfx | Font | Descent | Get font descent in pixels |
| m2gfx | Font | DrawText | Render text at position with color using font |
| m2gfx | Font | DrawTextWrapped | Render text with word wrapping at position with color |
| m2gfx | Font | GetStyle | Get current font style flags |
| m2gfx | Font | Height | Get font height in pixels |
| m2gfx | Font | LineSkip | Get recommended line skip in pixels |
| m2gfx | Font | Open | Open a font from file path at given point size |
| m2gfx | Font | SetHinting | Set font hinting mode |
| m2gfx | Font | SetStyle | Set font style flags (bold, italic, etc.) |
| m2gfx | Font | TextHeight | Get rendered text height in pixels |
| m2gfx | Font | TextWidth | Get rendered text width in pixels |
| m2gfx | Gfx | CreateRenderer | Create a renderer for a window with given flags |
| m2gfx | Gfx | CreateWindow | Create a window with title, size, and flags |
| m2gfx | Gfx | Delay | Delay execution for ms milliseconds |
| m2gfx | Gfx | DestroyRenderer | Destroy a renderer |
| m2gfx | Gfx | DestroyWindow | Destroy a window |
| m2gfx | Gfx | DisplayCount | Get number of displays |
| m2gfx | Gfx | GetClipboard | Get text from the system clipboard |
| m2gfx | Gfx | GetWindowHeight | Get window height in pixels |
| m2gfx | Gfx | GetWindowID | Get the unique ID of a window |
| m2gfx | Gfx | GetWindowWidth | Get window width in pixels |
| m2gfx | Gfx | HasClipboard | Check if clipboard has text content |
| m2gfx | Gfx | HideWindow | Hide a window |
| m2gfx | Gfx | Init | Initialize the graphics subsystem |
| m2gfx | Gfx | InitFont | Initialize the font subsystem |
| m2gfx | Gfx | MaximizeWindow | Maximize a window |
| m2gfx | Gfx | MinimizeWindow | Minimize a window |
| m2gfx | Gfx | OutputHeight | Get renderer output height in pixels |
| m2gfx | Gfx | OutputWidth | Get renderer output width in pixels |
| m2gfx | Gfx | Present | Present the renderer's back buffer to the screen |
| m2gfx | Gfx | Quit | Shut down the graphics subsystem |
| m2gfx | Gfx | QuitFont | Shut down the font subsystem |
| m2gfx | Gfx | RaiseWindow | Raise a window to the front |
| m2gfx | Gfx | RestoreWindow | Restore a window from minimized/maximized state |
| m2gfx | Gfx | ScreenHeight | Get primary screen height in pixels |
| m2gfx | Gfx | ScreenWidth | Get primary screen width in pixels |
| m2gfx | Gfx | SetClipboard | Set text to the system clipboard |
| m2gfx | Gfx | SetCursor | Set the cursor type |
| m2gfx | Gfx | SetFullscreen | Set window fullscreen mode |
| m2gfx | Gfx | SetTitle | Set window title text |
| m2gfx | Gfx | SetWindowMaxSize | Set maximum window size |
| m2gfx | Gfx | SetWindowMinSize | Set minimum window size |
| m2gfx | Gfx | SetWindowPos | Set window position on screen |
| m2gfx | Gfx | SetWindowSize | Set window size in pixels |
| m2gfx | Gfx | ShowCursor | Show or hide the cursor |
| m2gfx | Gfx | ShowWindow | Show a window |
| m2gfx | Gfx | Ticks | Get time in milliseconds since initialization |
| m2gfx | GfxBridge | gfx_alloc | Allocate memory of given byte size |
| m2gfx | GfxBridge | gfx_buf_get | Get byte value at offset in a buffer |
| m2gfx | GfxBridge | gfx_buf_set | Set byte value at offset in a buffer |
| m2gfx | GfxBridge | gfx_clear | Clear the renderer |
| m2gfx | GfxBridge | gfx_clear_clip | Clear the clipping rectangle |
| m2gfx | GfxBridge | gfx_close_font | Close a font handle |
| m2gfx | GfxBridge | gfx_create_renderer | Create a renderer for a window |
| m2gfx | GfxBridge | gfx_create_texture | Create a blank texture with given dimensions |
| m2gfx | GfxBridge | gfx_create_window | Create a window with title, size, and flags |
| m2gfx | GfxBridge | gfx_dealloc | Free previously allocated memory |
| m2gfx | GfxBridge | gfx_delay | Delay execution for ms milliseconds |
| m2gfx | GfxBridge | gfx_destroy_renderer | Destroy a renderer |
| m2gfx | GfxBridge | gfx_destroy_texture | Destroy a texture |
| m2gfx | GfxBridge | gfx_destroy_window | Destroy a window |
| m2gfx | GfxBridge | gfx_display_count | Get number of displays |
| m2gfx | GfxBridge | gfx_draw_line | Draw a line between two points |
| m2gfx | GfxBridge | gfx_draw_point | Draw a single pixel |
| m2gfx | GfxBridge | gfx_draw_rect | Draw a rectangle outline |
| m2gfx | GfxBridge | gfx_draw_text | Render text at position with color |
| m2gfx | GfxBridge | gfx_draw_text_wrapped | Render text with word wrapping |
| m2gfx | GfxBridge | gfx_draw_texture | Draw a texture at position |
| m2gfx | GfxBridge | gfx_draw_texture_ex | Draw a texture with source and destination rects |
| m2gfx | GfxBridge | gfx_draw_texture_rot | Draw a texture with rotation and flip |
| m2gfx | GfxBridge | gfx_event_key | Get key code from last event |
| m2gfx | GfxBridge | gfx_event_mod | Get modifier flags from last event |
| m2gfx | GfxBridge | gfx_event_mouse_btn | Get mouse button from last event |
| m2gfx | GfxBridge | gfx_event_mouse_x | Get mouse X from last event |
| m2gfx | GfxBridge | gfx_event_mouse_y | Get mouse Y from last event |
| m2gfx | GfxBridge | gfx_event_scancode | Get scan code from last event |
| m2gfx | GfxBridge | gfx_event_text | Get text input string into buffer |
| m2gfx | GfxBridge | gfx_event_text_len | Get text input string length |
| m2gfx | GfxBridge | gfx_event_wheel_x | Get horizontal scroll amount from last event |
| m2gfx | GfxBridge | gfx_event_wheel_y | Get vertical scroll amount from last event |
| m2gfx | GfxBridge | gfx_event_win_event | Get window event subtype |
| m2gfx | GfxBridge | gfx_event_win_id | Get window ID from last event |
| m2gfx | GfxBridge | gfx_fill_rect | Draw a filled rectangle |
| m2gfx | GfxBridge | gfx_font_ascent | Get font ascent |
| m2gfx | GfxBridge | gfx_font_descent | Get font descent |
| m2gfx | GfxBridge | gfx_font_get_style | Get current font style |
| m2gfx | GfxBridge | gfx_font_height | Get font height |
| m2gfx | GfxBridge | gfx_font_line_skip | Get font recommended line skip |
| m2gfx | GfxBridge | gfx_font_set_hinting | Set font hinting mode |
| m2gfx | GfxBridge | gfx_font_style | Set font style flags |
| m2gfx | GfxBridge | gfx_get_clip_h | Get clip rect height |
| m2gfx | GfxBridge | gfx_get_clip_w | Get clip rect width |
| m2gfx | GfxBridge | gfx_get_clip_x | Get clip rect X |
| m2gfx | GfxBridge | gfx_get_clip_y | Get clip rect Y |
| m2gfx | GfxBridge | gfx_get_clipboard | Get clipboard text into buffer |
| m2gfx | GfxBridge | gfx_get_color | Get current draw color RGBA |
| m2gfx | GfxBridge | gfx_get_win_height | Get window height |
| m2gfx | GfxBridge | gfx_get_win_id | Get window ID |
| m2gfx | GfxBridge | gfx_get_win_width | Get window width |
| m2gfx | GfxBridge | gfx_has_clipboard | Check if clipboard has text |
| m2gfx | GfxBridge | gfx_hide_win | Hide a window |
| m2gfx | GfxBridge | gfx_init | Initialize SDL graphics subsystem |
| m2gfx | GfxBridge | gfx_is_text_active | Check if text input mode is active |
| m2gfx | GfxBridge | gfx_key_state | Get key pressed state by scancode |
| m2gfx | GfxBridge | gfx_load_bmp | Load a BMP image as a texture |
| m2gfx | GfxBridge | gfx_log | Append a log message to a file |
| m2gfx | GfxBridge | gfx_maximize_win | Maximize a window |
| m2gfx | GfxBridge | gfx_minimize_win | Minimize a window |
| m2gfx | GfxBridge | gfx_mouse_global | Get global mouse position and button state |
| m2gfx | GfxBridge | gfx_mouse_state | Get mouse position and button state |
| m2gfx | GfxBridge | gfx_open_font | Open a TTF font at given size |
| m2gfx | GfxBridge | gfx_output_height | Get renderer output height |
| m2gfx | GfxBridge | gfx_output_width | Get renderer output width |
| m2gfx | GfxBridge | gfx_pb_clear | Clear pixel buffer with palette index |
| m2gfx | GfxBridge | gfx_pb_composite | Composite source pixel buffer onto destination |
| m2gfx | GfxBridge | gfx_pb_copy_pixels | Copy pixels from source to destination pixel buffer |
| m2gfx | GfxBridge | gfx_pb_create | Create a pixel buffer with given dimensions |
| m2gfx | GfxBridge | gfx_pb_fill_row | Fill a horizontal row of pixels with palette index |
| m2gfx | GfxBridge | gfx_pb_flush_tex | Flush RGBA pixel buffer to texture |
| m2gfx | GfxBridge | gfx_pb_free | Free a pixel buffer |
| m2gfx | GfxBridge | gfx_pb_free_save | Free a saved region |
| m2gfx | GfxBridge | gfx_pb_get | Get pixel palette index at coordinates |
| m2gfx | GfxBridge | gfx_pb_height | Get pixel buffer height |
| m2gfx | GfxBridge | gfx_pb_load_png | Load a PNG file as a pixel buffer |
| m2gfx | GfxBridge | gfx_pb_mark_dirty | Mark a rectangular region as dirty |
| m2gfx | GfxBridge | gfx_pb_pal_packed | Get packed RGBA value for palette entry |
| m2gfx | GfxBridge | gfx_pb_pal_to_screen | Convert palette-indexed buffer to RGBA screen buffer |
| m2gfx | GfxBridge | gfx_pb_pixel_ptr | Get pointer to raw pixel data |
| m2gfx | GfxBridge | gfx_pb_render | Render pixel buffer to texture |
| m2gfx | GfxBridge | gfx_pb_render_alpha | Render pixel buffer to texture with alpha |
| m2gfx | GfxBridge | gfx_pb_render_ham | Render pixel buffer in HAM (Hold-And-Modify) mode |
| m2gfx | GfxBridge | gfx_pb_restore | Restore a saved region to pixel buffer |
| m2gfx | GfxBridge | gfx_pb_rgba_get32 | Get 32-bit RGBA value at offset |
| m2gfx | GfxBridge | gfx_pb_rgba_set32 | Set 32-bit RGBA value at offset |
| m2gfx | GfxBridge | gfx_pb_save | Save a rectangular region from pixel buffer |
| m2gfx | GfxBridge | gfx_pb_save_h | Get saved region height |
| m2gfx | GfxBridge | gfx_pb_save_png | Save pixel buffer as PNG file |
| m2gfx | GfxBridge | gfx_pb_save_w | Get saved region width |
| m2gfx | GfxBridge | gfx_pb_set | Set pixel palette index at coordinates |
| m2gfx | GfxBridge | gfx_pb_set_pal | Set palette entry RGB values |
| m2gfx | GfxBridge | gfx_pb_stamp_text | Stamp text onto pixel buffer using font |
| m2gfx | GfxBridge | gfx_pb_total | Get total pixel count in buffer |
| m2gfx | GfxBridge | gfx_pb_width | Get pixel buffer width |
| m2gfx | GfxBridge | gfx_poll_event | Poll for pending SDL events |
| m2gfx | GfxBridge | gfx_present | Present renderer back buffer |
| m2gfx | GfxBridge | gfx_quit | Shut down SDL |
| m2gfx | GfxBridge | gfx_raise_win | Raise a window |
| m2gfx | GfxBridge | gfx_reset_target | Reset render target to default |
| m2gfx | GfxBridge | gfx_reset_viewport | Reset viewport to full area |
| m2gfx | GfxBridge | gfx_restore_win | Restore a window |
| m2gfx | GfxBridge | gfx_screen_height | Get primary screen height |
| m2gfx | GfxBridge | gfx_screen_width | Get primary screen width |
| m2gfx | GfxBridge | gfx_set_blend | Set blend mode |
| m2gfx | GfxBridge | gfx_set_clip | Set clipping rectangle |
| m2gfx | GfxBridge | gfx_set_clipboard | Set clipboard text |
| m2gfx | GfxBridge | gfx_set_color | Set draw color RGBA |
| m2gfx | GfxBridge | gfx_set_cursor | Set cursor type |
| m2gfx | GfxBridge | gfx_set_fullscreen | Set window fullscreen mode |
| m2gfx | GfxBridge | gfx_set_target | Set render target to texture |
| m2gfx | GfxBridge | gfx_set_tex_alpha | Set texture alpha modulation |
| m2gfx | GfxBridge | gfx_set_tex_blend | Set texture blend mode |
| m2gfx | GfxBridge | gfx_set_tex_color | Set texture color modulation |
| m2gfx | GfxBridge | gfx_set_title | Set window title |
| m2gfx | GfxBridge | gfx_set_viewport | Set viewport rectangle |
| m2gfx | GfxBridge | gfx_set_win_max_size | Set window maximum size |
| m2gfx | GfxBridge | gfx_set_win_min_size | Set window minimum size |
| m2gfx | GfxBridge | gfx_set_win_pos | Set window position |
| m2gfx | GfxBridge | gfx_set_win_size | Set window size |
| m2gfx | GfxBridge | gfx_show_cursor | Show or hide cursor |
| m2gfx | GfxBridge | gfx_show_win | Show a window |
| m2gfx | GfxBridge | gfx_start_text | Start text input mode |
| m2gfx | GfxBridge | gfx_stop_text | Stop text input mode |
| m2gfx | GfxBridge | gfx_tex_height | Get texture height |
| m2gfx | GfxBridge | gfx_tex_width | Get texture width |
| m2gfx | GfxBridge | gfx_text_height | Get rendered text height |
| m2gfx | GfxBridge | gfx_text_texture | Create a texture from rendered text |
| m2gfx | GfxBridge | gfx_text_width | Get rendered text width |
| m2gfx | GfxBridge | gfx_ticks | Get ticks since init |
| m2gfx | GfxBridge | gfx_ttf_init | Initialize TTF font subsystem |
| m2gfx | GfxBridge | gfx_ttf_quit | Shut down TTF font subsystem |
| m2gfx | GfxBridge | gfx_wait_event | Wait for an SDL event |
| m2gfx | GfxBridge | gfx_wait_event_timeout | Wait for event with timeout |
| m2gfx | GfxBridge | gfx_warp_mouse | Warp mouse cursor position |
| m2gfx | PixBuf | AntiAlias | Apply anti-aliasing to a rectangular region |
| m2gfx | PixBuf | Bezier | Draw a cubic Bezier curve on pixel buffer |
| m2gfx | PixBuf | Circle | Draw a circle outline on pixel buffer |
| m2gfx | PixBuf | Clear | Clear pixel buffer with palette index |
| m2gfx | PixBuf | ConfigLoad | Load configuration key-value pairs from file |
| m2gfx | PixBuf | ConfigSave | Save configuration key-value pairs to file |
| m2gfx | PixBuf | CopperGradient | Render copper-style gradient effect on scan lines |
| m2gfx | PixBuf | CopyRegion | Copy a rectangular region within pixel buffer |
| m2gfx | PixBuf | Create | Create a pixel buffer with given dimensions |
| m2gfx | PixBuf | DitherFill | Fill a region with dithered pattern |
| m2gfx | PixBuf | Ellipse | Draw an ellipse outline on pixel buffer |
| m2gfx | PixBuf | FillCircle | Draw a filled circle on pixel buffer |
| m2gfx | PixBuf | FillEllipse | Draw a filled ellipse on pixel buffer |
| m2gfx | PixBuf | FillRect | Draw a filled rectangle on pixel buffer |
| m2gfx | PixBuf | FillTriangle | Draw a filled triangle on pixel buffer |
| m2gfx | PixBuf | FlipH | Flip a region horizontally |
| m2gfx | PixBuf | FlipV | Flip a region vertically |
| m2gfx | PixBuf | FloodFill | Flood fill from a point with palette index |
| m2gfx | PixBuf | FrameCount | Get total number of animation frames |
| m2gfx | PixBuf | FrameCurrent | Get current animation frame index |
| m2gfx | PixBuf | FrameDelete | Delete an animation frame by index |
| m2gfx | PixBuf | FrameDuplicate | Duplicate an animation frame |
| m2gfx | PixBuf | FrameGet | Get pixel buffer for animation frame by index |
| m2gfx | PixBuf | FrameGetCurrent | Get pixel buffer for current animation frame |
| m2gfx | PixBuf | FrameInit | Initialize animation frame system |
| m2gfx | PixBuf | FrameNew | Create a new animation frame with given dimensions |
| m2gfx | PixBuf | FrameSet | Set current animation frame by index |
| m2gfx | PixBuf | FrameSetTiming | Set timing (ms) for an animation frame |
| m2gfx | PixBuf | FramesToSheet | Combine all frames into a sprite sheet |
| m2gfx | PixBuf | FrameTiming | Get timing (ms) for an animation frame |
| m2gfx | PixBuf | Free | Free a pixel buffer |
| m2gfx | PixBuf | FreeSave | Free a saved region |
| m2gfx | PixBuf | GetPix | Get pixel palette index at coordinates |
| m2gfx | PixBuf | Gradient | Fill a region with linear gradient between two palette colors |
| m2gfx | PixBuf | GradientAngle | Fill a region with angled gradient between two palette colors |
| m2gfx | PixBuf | Height | Get pixel buffer height |
| m2gfx | PixBuf | LayerActive | Get active layer index |
| m2gfx | PixBuf | LayerAdd | Add a new layer with given dimensions; returns index |
| m2gfx | PixBuf | LayerCount | Get total number of layers |
| m2gfx | PixBuf | LayerFlatten | Flatten all layers onto destination with transparency |
| m2gfx | PixBuf | LayerGet | Get pixel buffer for layer by index |
| m2gfx | PixBuf | LayerGetActive | Get pixel buffer for the active layer |
| m2gfx | PixBuf | LayerInit | Initialize the layer system |
| m2gfx | PixBuf | LayerMoveDown | Move a layer down in the stack |
| m2gfx | PixBuf | LayerMoveUp | Move a layer up in the stack |
| m2gfx | PixBuf | LayerRemove | Remove a layer by index |
| m2gfx | PixBuf | LayerSetActive | Set the active layer by index |
| m2gfx | PixBuf | LayerSetVisible | Set layer visibility |
| m2gfx | PixBuf | LayerVisible | Check if a layer is visible |
| m2gfx | PixBuf | Line | Draw a line on pixel buffer |
| m2gfx | PixBuf | LinePerfect | Draw a pixel-perfect line on pixel buffer |
| m2gfx | PixBuf | LoadDP2 | Load a .dp2 project file |
| m2gfx | PixBuf | LoadPal | Load a palette from file into pixel buffer |
| m2gfx | PixBuf | LoadPNG | Load a PNG file as a pixel buffer |
| m2gfx | PixBuf | Log | Append a log message to a file |
| m2gfx | PixBuf | NearestColor | Find the nearest palette color to given RGB |
| m2gfx | PixBuf | PalB | Get blue component of palette entry |
| m2gfx | PixBuf | PalG | Get green component of palette entry |
| m2gfx | PixBuf | PalPacked | Get packed RGBA value for palette entry |
| m2gfx | PixBuf | PalR | Get red component of palette entry |
| m2gfx | PixBuf | PatternFill | Fill a region with 4x4 Bayer dither pattern |
| m2gfx | PixBuf | PolyAdd | Add a vertex to the polygon |
| m2gfx | PixBuf | PolyCount | Get number of vertices in the polygon |
| m2gfx | PixBuf | PolyDraw | Draw polygon outline on pixel buffer |
| m2gfx | PixBuf | PolyFill | Draw filled polygon on pixel buffer |
| m2gfx | PixBuf | PolyReset | Reset polygon vertex list |
| m2gfx | PixBuf | PolyX | Get X coordinate of polygon vertex by index |
| m2gfx | PixBuf | PolyY | Get Y coordinate of polygon vertex by index |
| m2gfx | PixBuf | Rect | Draw a rectangle outline on pixel buffer |
| m2gfx | PixBuf | Render | Render pixel buffer to SDL texture |
| m2gfx | PixBuf | RenderAlpha | Render pixel buffer to texture with alpha transparency |
| m2gfx | PixBuf | RenderHAM | Render pixel buffer in HAM (Hold-And-Modify) mode |
| m2gfx | PixBuf | ReplaceColor | Replace all pixels of one color with another |
| m2gfx | PixBuf | Restore | Restore a saved region to pixel buffer |
| m2gfx | PixBuf | Rotate180 | Rotate a region 180 degrees |
| m2gfx | PixBuf | Rotate270 | Rotate a region 270 degrees clockwise |
| m2gfx | PixBuf | Rotate90 | Rotate a region 90 degrees clockwise |
| m2gfx | PixBuf | Save | Save a rectangular region for undo |
| m2gfx | PixBuf | SaveBMP | Save pixel buffer as BMP file |
| m2gfx | PixBuf | SaveDP2 | Save project as .dp2 file |
| m2gfx | PixBuf | SaveH | Get saved region height |
| m2gfx | PixBuf | SavePal | Save pixel buffer palette to file |
| m2gfx | PixBuf | SavePNG | Save pixel buffer as PNG file |
| m2gfx | PixBuf | SaveW | Get saved region width |
| m2gfx | PixBuf | SetPal | Set palette entry RGB values |
| m2gfx | PixBuf | SetPix | Set pixel palette index at coordinates |
| m2gfx | PixBuf | StampText | Render text string onto pixel buffer using font |
| m2gfx | PixBuf | ThickLine | Draw a line with specified thickness on pixel buffer |
| m2gfx | PixBuf | Triangle | Draw a triangle outline on pixel buffer |
| m2gfx | PixBuf | Width | Get pixel buffer width |
| m2gfx | Texture | Create | Create a blank texture with given dimensions |
| m2gfx | Texture | Destroy | Destroy a texture |
| m2gfx | Texture | Draw | Draw a texture at position |
| m2gfx | Texture | DrawRegion | Draw a texture region with source and destination rects |
| m2gfx | Texture | DrawRotated | Draw a texture with rotation and flip |
| m2gfx | Texture | FromText | Create a texture from rendered text with font and color |
| m2gfx | Texture | Height | Get texture height |
| m2gfx | Texture | LoadBMP | Load a BMP image as a texture |
| m2gfx | Texture | ResetTarget | Reset render target to default |
| m2gfx | Texture | SetAlpha | Set texture alpha modulation |
| m2gfx | Texture | SetBlendMode | Set texture blend mode |
| m2gfx | Texture | SetColorMod | Set texture color modulation RGB |
| m2gfx | Texture | SetTarget | Set render target to this texture |
| m2gfx | Texture | Width | Get texture width |
| m2glob | Glob | HasPathSep | Returns TRUE if pattern contains '/' anywhere |
| m2glob | Glob | IsAnchored | Returns TRUE if pattern starts with '/' |
| m2glob | Glob | IsNegated | Returns TRUE if pattern starts with '!' |
| m2glob | Glob | Match | Full glob match of pattern against text |
| m2glob | Glob | StripAnchor | Copy pattern without leading '/' into out |
| m2glob | Glob | StripNegation | Copy pattern without leading '!' into out |
| m2http | Buffers | AdvanceWrite | Advance write position after external writes |
| m2http | Buffers | AppendByte | Append a single byte to the buffer |
| m2http | Buffers | AppendBytes | Append bytes from a CHAR array to the buffer |
| m2http | Buffers | AppendString | Append a NUL-terminated string to the buffer |
| m2http | Buffers | Capacity | Return current buffer capacity |
| m2http | Buffers | Clear | Reset buffer to empty state |
| m2http | Buffers | Compact | Slide unread bytes to offset 0 |
| m2http | Buffers | Consume | Consume n bytes from the read position |
| m2http | Buffers | CopyOut | Copy bytes from buffer at offset into destination array |
| m2http | Buffers | Create | Create a buffer with initial capacity and growth mode |
| m2http | Buffers | Destroy | Destroy a buffer and free its memory |
| m2http | Buffers | FindByte | Search for a byte in the buffer; returns position if found |
| m2http | Buffers | FindCRLF | Search for CRLF sequence in the buffer |
| m2http | Buffers | Length | Return number of readable bytes in the buffer |
| m2http | Buffers | PeekByte | Read a byte at offset without consuming it |
| m2http | Buffers | Remaining | Return number of writable bytes remaining |
| m2http | Buffers | SliceLen | Return number of readable bytes (zero-copy) |
| m2http | Buffers | SlicePtr | Return pointer to first readable byte (zero-copy) |
| m2http | Buffers | WritePtr | Return pointer to first writable byte (zero-copy) |
| m2http | DNS | ResolveA | Resolve hostname to IPv4 address; returns a Future |
| m2http | DnsBridge | m2_connect_ipv4 | Connect socket to IPv4 address and port |
| m2http | DnsBridge | m2_dns_errno | Return errno from last DNS operation |
| m2http | DnsBridge | m2_dns_resolve_a | Resolve hostname to IPv4 address via getaddrinfo |
| m2http | DnsBridge | m2_dns_strerror | Get error string for DNS errno |
| m2http | DnsBridge | m2_getsockopt_error | Get socket error via getsockopt |
| m2http | H2Client | FreeResponse | Free a response and its body buffer |
| m2http | H2Client | Get | Issue an HTTP/2 GET request; returns a Future |
| m2http | H2Client | Put | Issue an HTTP/2 PUT request with body; returns a Future |
| m2http | H2Client | SetSkipVerify | Control TLS peer verification globally |
| m2http | HTTPClient | FindHeader | Case-insensitive header lookup in a response |
| m2http | HTTPClient | FreeResponse | Free a response and its body buffer |
| m2http | HTTPClient | Get | Issue an HTTP GET request; returns a Future |
| m2http | HTTPClient | Head | Issue an HTTP HEAD request; returns a Future |
| m2http | HTTPClient | Put | Issue an HTTP PUT request with body; returns a Future |
| m2http | HTTPClient | SetSkipVerify | Control TLS peer verification globally |
| m2http | URI | DefaultPort | Return default port for a scheme (http=80, https=443) |
| m2http | URI | Parse | Parse a URI string into components |
| m2http | URI | PercentDecode | Decode percent-encoded sequences in a string |
| m2http | URI | RequestPath | Build the HTTP request path from a URIRec |
| m2http2 | Http2Conn | ApplyRemoteSettings | Apply received settings to remote settings |
| m2http2 | Http2Conn | ClearOutput | Clear the output buffer after flushing |
| m2http2 | Http2Conn | ConsumeConnSendWindow | Consume from connection-level send window |
| m2http2 | Http2Conn | FindStream | Find stream by ID; returns slot index or MaxStreams |
| m2http2 | Http2Conn | FreeConn | Free connection resources |
| m2http2 | Http2Conn | GetOutput | Get a view of pending output data to send |
| m2http2 | Http2Conn | InitConn | Initialise a connection with default settings |
| m2http2 | Http2Conn | OpenStream | Allocate a new client stream; returns stream ID |
| m2http2 | Http2Conn | ProcessFrame | Process a received frame and update state |
| m2http2 | Http2Conn | SendPreface | Write client connection preface and SETTINGS to output |
| m2http2 | Http2Conn | UpdateConnSendWindow | Update connection-level send window from WINDOW_UPDATE |
| m2http2 | Http2Frame | CheckPreface | Check if bytes match the HTTP/2 connection preface |
| m2http2 | Http2Frame | DecodeGoaway | Decode a GOAWAY frame payload |
| m2http2 | Http2Frame | DecodeHeader | Decode a 9-byte frame header from a BytesView |
| m2http2 | Http2Frame | DecodeRstStream | Decode a RST_STREAM payload (4 bytes) |
| m2http2 | Http2Frame | DecodeSettings | Decode a SETTINGS frame payload into a Settings record |
| m2http2 | Http2Frame | DecodeWindowUpdate | Decode a WINDOW_UPDATE payload (4 bytes) |
| m2http2 | Http2Frame | EncodeDataHeader | Encode a DATA frame header |
| m2http2 | Http2Frame | EncodeGoaway | Encode a GOAWAY frame |
| m2http2 | Http2Frame | EncodeHeader | Encode a 9-byte frame header into a Buf |
| m2http2 | Http2Frame | EncodeHeadersHeader | Encode a HEADERS frame header |
| m2http2 | Http2Frame | EncodePing | Encode a PING frame with 8 bytes of opaque data |
| m2http2 | Http2Frame | EncodeRstStream | Encode a RST_STREAM frame |
| m2http2 | Http2Frame | EncodeSettings | Encode a full SETTINGS frame with all standard settings |
| m2http2 | Http2Frame | EncodeSettingsAck | Encode a SETTINGS ACK frame (no payload) |
| m2http2 | Http2Frame | EncodeWindowUpdate | Encode a WINDOW_UPDATE frame |
| m2http2 | Http2Frame | WritePreface | Write the 24-byte HTTP/2 client connection preface |
| m2http2 | Http2Hpack | DecodeHeaderBlock | Decode a complete header block from a BytesView |
| m2http2 | Http2Hpack | DecodeInt | Decode an HPACK integer with given prefix bit width |
| m2http2 | Http2Hpack | DynCount | Return number of entries in the dynamic table |
| m2http2 | Http2Hpack | DynInit | Initialise a dynamic table with given max size |
| m2http2 | Http2Hpack | DynInsert | Insert a header entry at the front of the dynamic table |
| m2http2 | Http2Hpack | DynLookup | Look up entry by dynamic table index |
| m2http2 | Http2Hpack | DynResize | Resize the dynamic table; evicts as needed |
| m2http2 | Http2Hpack | EncodeHeaderBlock | Encode headers into a header block appended to Buf |
| m2http2 | Http2Hpack | EncodeInt | Encode an HPACK integer with given prefix bit width |
| m2http2 | Http2Hpack | StaticFind | Find a static table entry matching name and optionally value |
| m2http2 | Http2Hpack | StaticLookup | Look up a static table entry by 1-based index |
| m2http2 | Http2Stream | ConsumeRecvWindow | Consume bytes from the stream receive window |
| m2http2 | Http2Stream | ConsumeSendWindow | Consume bytes from the stream send window |
| m2http2 | Http2Stream | InitStream | Initialise a stream with ID, window size, and transition table |
| m2http2 | Http2Stream | InitStreamTable | Initialise the shared stream transition table per RFC 7540 |
| m2http2 | Http2Stream | IsClosed | Check if the stream is in Closed state |
| m2http2 | Http2Stream | StreamState | Return current FSM state as a StateId |
| m2http2 | Http2Stream | StreamStep | Process a stream event; returns the step status |
| m2http2 | Http2Stream | UpdateRecvWindow | Add increment to the stream receive window |
| m2http2 | Http2Stream | UpdateSendWindow | Add increment to the stream send window from WINDOW_UPDATE |
| m2http2 | Http2TestUtil | BuildFrame | Build a raw frame with header and payload bytes |
| m2http2 | Http2TestUtil | BuildGoawayFrame | Build a GOAWAY frame |
| m2http2 | Http2TestUtil | BuildPingFrame | Build a PING frame with 8 bytes of data |
| m2http2 | Http2TestUtil | BuildRstStreamFrame | Build a RST_STREAM frame |
| m2http2 | Http2TestUtil | BuildSettingsAckFrame | Build a SETTINGS ACK frame |
| m2http2 | Http2TestUtil | BuildSettingsFrame | Build a SETTINGS frame from settings values |
| m2http2 | Http2TestUtil | BuildWindowUpdateFrame | Build a WINDOW_UPDATE frame |
| m2http2 | Http2TestUtil | ReadFrameHeader | Read a frame header from the start of a view |
| m2http2 | Http2TestUtil | ReadFramePayload | Extract payload from a view given a frame header |
| m2http2 | Http2Types | InitDefaultSettings | Initialise a Settings record with default values |
| m2http2server | Http2Middleware | ChainAdd | Add middleware to the chain; returns FALSE if full |
| m2http2server | Http2Middleware | ChainInit | Initialize a middleware chain |
| m2http2server | Http2Middleware | ChainRun | Run middleware chain then call handler if all pass |
| m2http2server | Http2Middleware | GuardMw | Catch handler errors and return 500 |
| m2http2server | Http2Middleware | LoggingMw | Log request method and path at INFO level |
| m2http2server | Http2Middleware | SizeLimitMw | Reject request bodies exceeding size limit |
| m2http2server | Http2Router | AddRoute | Register a handler for method+path; returns FALSE if full |
| m2http2server | Http2Router | Dispatch | Dispatch request to matching handler or 404 |
| m2http2server | Http2Router | RouterInit | Initialize a router |
| m2http2server | Http2Server | AddMiddleware | Add middleware to the pre-handler chain |
| m2http2server | Http2Server | AddRoute | Register a route with exact match on method+path |
| m2http2server | Http2Server | Create | Create a server with given options (TLS, socket, router) |
| m2http2server | Http2Server | Destroy | Destroy the server and free all resources |
| m2http2server | Http2Server | Drain | Initiate graceful shutdown with GOAWAY and drain timeout |
| m2http2server | Http2Server | Start | Start the server; blocks until Stop or Drain completes |
| m2http2server | Http2Server | Stop | Force-stop the server immediately |
| m2http2server | Http2ServerConn | ConnClose | Close and destroy connection, freeing all resources |
| m2http2server | Http2ServerConn | ConnCreate | Create a new connection from an accepted socket |
| m2http2server | Http2ServerConn | ConnCreateTest | Create a test connection with no TLS/Stream (in-memory) |
| m2http2server | Http2ServerConn | ConnDrain | Initiate graceful shutdown by sending GOAWAY |
| m2http2server | Http2ServerConn | ConnFeedBytes | Feed raw bytes for testing (bypasses Stream/TLS) |
| m2http2server | Http2ServerConn | ConnFlush | Flush any pending write data |
| m2http2server | Http2ServerConn | ConnOnEvent | EventLoop watcher callback for connection I/O |
| m2http2server | Http2ServerConn | SetConnCleanup | Set the connection cleanup callback |
| m2http2server | Http2ServerConn | SetServerDispatch | Set the server dispatch callback for request handling |
| m2http2server | Http2ServerLog | LogConn | Log connection event (accepted, closed, error) |
| m2http2server | Http2ServerLog | LogInit | Initialise a logger with "h2server" category |
| m2http2server | Http2ServerLog | LogProtocol | Log protocol event (SETTINGS, GOAWAY) |
| m2http2server | Http2ServerLog | LogRequest | Log request completion with method, path, status, duration |
| m2http2server | Http2ServerMetrics | AddBytesIn | Add to incoming bytes counter |
| m2http2server | Http2ServerMetrics | AddBytesOut | Add to outgoing bytes counter |
| m2http2server | Http2ServerMetrics | DecConnsActive | Decrement active connections counter |
| m2http2server | Http2ServerMetrics | IncALPNReject | Increment ALPN rejection counter |
| m2http2server | Http2ServerMetrics | IncConnsAccepted | Increment accepted connections counter |
| m2http2server | Http2ServerMetrics | IncConnsActive | Increment active connections counter |
| m2http2server | Http2ServerMetrics | IncConnsClosed | Increment closed connections counter |
| m2http2server | Http2ServerMetrics | IncProtoErrors | Increment protocol errors counter |
| m2http2server | Http2ServerMetrics | IncReqTotal | Increment total requests counter |
| m2http2server | Http2ServerMetrics | IncResp | Increment response counter by status code bucket |
| m2http2server | Http2ServerMetrics | IncStreamsOpened | Increment opened streams counter |
| m2http2server | Http2ServerMetrics | IncTLSFail | Increment TLS handshake failure counter |
| m2http2server | Http2ServerMetrics | MetricsInit | Initialize all metrics counters to zero |
| m2http2server | Http2ServerMetrics | MetricsLog | Log all counters at INFO level |
| m2http2server | Http2ServerStream | AccumulateData | Accumulate DATA frame payload into request body |
| m2http2server | Http2ServerStream | AllocSlot | Find an unused slot and initialise for a stream ID |
| m2http2server | Http2ServerStream | AssembleHeaders | Extract pseudo-headers and regular headers into request |
| m2http2server | Http2ServerStream | FindSlot | Find slot by stream ID; returns index or MaxStreamSlots |
| m2http2server | Http2ServerStream | FlushData | Flush remaining buffered response DATA for a slot |
| m2http2server | Http2ServerStream | SendResponse | Encode response as HEADERS + DATA frames into output buffer |
| m2http2server | Http2ServerStream | SlotFree | Release a slot after response is complete |
| m2http2server | Http2ServerStream | SlotInit | Initialize a stream slot |
| m2http2server | Http2ServerTestUtil | BuildClientPreface | Append the 24-byte client connection preface to buffer |
| m2http2server | Http2ServerTestUtil | BuildContinuation | Append a raw CONTINUATION frame for violation testing |
| m2http2server | Http2ServerTestUtil | BuildData | Append a DATA frame to buffer |
| m2http2server | Http2ServerTestUtil | BuildGET | Build a minimal GET request HEADERS frame |
| m2http2server | Http2ServerTestUtil | BuildGoaway | Append a GOAWAY frame to buffer |
| m2http2server | Http2ServerTestUtil | BuildHeaders | Append a HEADERS frame with HPACK-encoded headers |
| m2http2server | Http2ServerTestUtil | BuildPing | Append a PING frame to buffer |
| m2http2server | Http2ServerTestUtil | BuildPOST | Build a POST request HEADERS frame (no END_STREAM) |
| m2http2server | Http2ServerTestUtil | BuildRstStream | Append a RST_STREAM frame to buffer |
| m2http2server | Http2ServerTestUtil | BuildSettings | Append a SETTINGS frame with given settings |
| m2http2server | Http2ServerTestUtil | BuildSettingsAck | Append a SETTINGS ACK frame to buffer |
| m2http2server | Http2ServerTestUtil | BuildWindowUpdate | Append a WINDOW_UPDATE frame to buffer |
| m2http2server | Http2ServerTestUtil | DoTestHandshake | Perform a complete H2 handshake on a test connection |
| m2http2server | Http2ServerTestUtil | FeedAndCollect | Feed raw bytes into test connection and collect output |
| m2http2server | Http2ServerTestUtil | ReadNextFrame | Parse the next frame from a BytesView |
| m2http2server | Http2ServerTypes | FreeRequest | Free request resources including body buffer |
| m2http2server | Http2ServerTypes | FreeResponse | Free response resources including body buffer |
| m2http2server | Http2ServerTypes | InitDefaultOpts | Initialize server options with default values |
| m2http2server | Http2ServerTypes | InitRequest | Initialize a request record |
| m2http2server | Http2ServerTypes | InitResponse | Initialize a response record |
| m2log | Log | AddSink | Add a sink to the logger; returns FALSE if full |
| m2log | Log | Debug | Log a message at DEBUG level |
| m2log | Log | DebugD | Log a message at DEBUG level using default logger |
| m2log | Log | Error | Log a message at ERROR level |
| m2log | Log | ErrorD | Log a message at ERROR level using default logger |
| m2log | Log | Fatal | Log a message at FATAL level |
| m2log | Log | FatalD | Log a message at FATAL level using default logger |
| m2log | Log | Format | Format a log record into a line buffer |
| m2log | Log | GetDropCount | Return number of log calls suppressed by recursion guard |
| m2log | Log | Info | Log a message at INFO level |
| m2log | Log | InfoD | Log a message at INFO level using default logger |
| m2log | Log | Init | Initialize a logger with level INFO and no sinks |
| m2log | Log | InitDefault | Initialize the default logger with console sink at INFO |
| m2log | Log | KVBool | Build a boolean field for structured logging |
| m2log | Log | KVInt | Build an integer field for structured logging |
| m2log | Log | KVStr | Build a string field for structured logging |
| m2log | Log | LogKV | Log a structured message with key/value fields |
| m2log | Log | LogMsg | Log a plain message at the given level |
| m2log | Log | MakeConsoleSink | Create a sink that writes formatted lines to stdout |
| m2log | Log | SetCategory | Set the category string for the logger |
| m2log | Log | SetLevel | Set the minimum log level for the logger |
| m2log | Log | Trace | Log a message at TRACE level |
| m2log | Log | TraceD | Log a message at TRACE level using default logger |
| m2log | Log | Warn | Log a message at WARN level |
| m2log | Log | WarnD | Log a message at WARN level using default logger |
| m2log | Log | WithCategory | Create a logger copy with a different category |
| m2log | LogSinkFile | Close | Close the file handle associated with a file sink |
| m2log | LogSinkFile | Create | Open a file for append and return a configured Sink |
| m2log | LogSinkThreadSafe | Close | Close file (if file sink) and destroy mutex |
| m2log | LogSinkThreadSafe | CreateFile | Open file for append; return mutex-protected sink |
| m2log | LogSinkThreadSafe | CreateStderr | Create a mutex-protected sink that writes to stderr |
| m2log | LogSinkMemory | Clear | Clear all stored lines and reset counters |
| m2log | LogSinkMemory | Contains | Check if any stored line contains the given substring |
| m2log | LogSinkMemory | Create | Initialize a MemorySink and return a Sink handle |
| m2log | LogSinkMemory | GetCount | Return number of lines currently stored |
| m2log | LogSinkMemory | GetLine | Copy stored line at index into buffer |
| m2log | LogSinkMemory | GetTotal | Return total number of log calls seen |
| m2log | LogSinkStream | Create | Create a stream sink that writes to an existing Stream |
| m2log | Sys | m2sys_fclose | Close a file handle |
| m2log | Sys | m2sys_fopen | Open a file with given path and mode |
| m2log | Sys | m2sys_fwrite_str | Write a string to a file handle |
| m2metrics | Metrics | Snapshot | Fill a SysSnapshot record with current system metrics (load, memory, CPU, RSS) |
| m2path | Path | Extension | Return the file extension including leading dot |
| m2path | Path | IsAbsolute | Return TRUE if path starts with "/" |
| m2path | Path | Join | Join two path components with "/" |
| m2path | Path | Match | Simple glob matching on the basename of a path |
| m2path | Path | Normalize | Collapse "." and ".." segments, strip trailing "/" |
| m2path | Path | RelativeTo | Compute a relative path from base to target |
| m2path | Path | Split | Split path into directory and base name |
| m2path | Path | StripExt | Remove the extension from the path |
| m2rpc | RpcClient | Call | Issue an RPC request; returns a Future |
| m2rpc | RpcClient | CancelAll | Cancel all pending calls with Closed error |
| m2rpc | RpcClient | FreeClient | Free internal buffers (does not close transport) |
| m2rpc | RpcClient | InitClient | Initialize a client with transport, scheduler, and event loop |
| m2rpc | RpcClient | OnReadable | Process incoming data and dispatch responses to promises |
| m2rpc | RpcCodec | DecodeError | Decode an RPC error message body |
| m2rpc | RpcCodec | DecodeHeader | Decode the common 6-byte header from a frame payload |
| m2rpc | RpcCodec | DecodeRequest | Decode an RPC request message body |
| m2rpc | RpcCodec | DecodeResponse | Decode an RPC response message body |
| m2rpc | RpcCodec | EncodeError | Encode an RPC error message into buffer |
| m2rpc | RpcCodec | EncodeRequest | Encode an RPC request into buffer |
| m2rpc | RpcCodec | EncodeResponse | Encode an RPC response into buffer |
| m2rpc | RpcErrors | ToString | Return human-readable string for a framework error code |
| m2rpc | RpcFrame | FreeFrameReader | Free the frame reader's internal buffer |
| m2rpc | RpcFrame | InitFrameReader | Initialize a frame reader with max size and transport |
| m2rpc | RpcFrame | ResetFrameReader | Reset the reader to initial state, discarding partial frame |
| m2rpc | RpcFrame | TryReadFrame | Attempt to read a complete frame; call repeatedly |
| m2rpc | RpcFrame | WriteFrame | Write a complete frame in a blocking loop |
| m2rpc | RpcServer | FreeServer | Free server internal buffers |
| m2rpc | RpcServer | InitServer | Initialize server with read/write transport functions |
| m2rpc | RpcServer | RegisterHandler | Register a handler for an RPC method name |
| m2rpc | RpcServer | ServeOnce | Process one incoming request and dispatch to handler |
| m2rpc | RpcTest | CloseA | Close endpoint A's write direction |
| m2rpc | RpcTest | CloseB | Close endpoint B's write direction |
| m2rpc | RpcTest | CreatePipe | Create an in-memory duplex pipe with optional I/O limits |
| m2rpc | RpcTest | DestroyPipe | Destroy a pipe and free its internal buffers |
| m2rpc | RpcTest | PendingAtoB | Return unread bytes pending in A-to-B direction |
| m2rpc | RpcTest | PendingBtoA | Return unread bytes pending in B-to-A direction |
| m2rpc | RpcTest | ReadA | Read from B-to-A direction (matches ReadFn signature) |
| m2rpc | RpcTest | ReadB | Read from A-to-B direction (matches ReadFn signature) |
| m2rpc | RpcTest | WriteA | Write to A-to-B direction (matches WriteFn signature) |
| m2rpc | RpcTest | WriteB | Write to B-to-A direction (matches WriteFn signature) |
| m2sockets | Sockets | Accept | Accept one incoming connection; returns client fd and peer address |
| m2sockets | Sockets | Bind | Bind socket to INADDR_ANY on the given port |
| m2sockets | Sockets | CloseSocket | Close a socket (idempotent if InvalidSocket) |
| m2sockets | Sockets | Connect | Resolve host and connect a TCP socket |
| m2sockets | Sockets | GetLastErrno | Return raw errno from the last failed bridge call |
| m2sockets | Sockets | GetLastErrorText | Copy strerror text into output buffer |
| m2sockets | Sockets | Listen | Mark socket as passive with given backlog |
| m2sockets | Sockets | RecvBytes | Receive up to max bytes into buffer |
| m2sockets | Sockets | RecvLine | Read until LF or buffer full; strips trailing CR+LF |
| m2sockets | Sockets | SendBytes | Send up to len bytes from buffer |
| m2sockets | Sockets | SendString | Send a NUL-terminated string (excluding the NUL) |
| m2sockets | Sockets | SetNonBlocking | Set or clear O_NONBLOCK on the socket |
| m2sockets | Sockets | Shutdown | Half-close a socket with given shutdown mode |
| m2sockets | Sockets | SocketCreate | Create a socket with given family and type |
| m2sockets | SocketsBridge | m2_accept | Accept a connection; returns fd, family, port, address |
| m2sockets | SocketsBridge | m2_bind_any | Bind socket to any address on given port |
| m2sockets | SocketsBridge | m2_close | Close a file descriptor |
| m2sockets | SocketsBridge | m2_connect_host_port | Connect socket to host and port |
| m2sockets | SocketsBridge | m2_errno | Return errno from last socket operation |
| m2sockets | SocketsBridge | m2_listen | Start listening on a socket with backlog |
| m2sockets | SocketsBridge | m2_recv | Receive bytes from a socket |
| m2sockets | SocketsBridge | m2_send | Send bytes on a socket |
| m2sockets | SocketsBridge | m2_set_nonblocking | Set or clear non-blocking mode on fd |
| m2sockets | SocketsBridge | m2_set_reuseaddr | Set or clear SO_REUSEADDR on fd |
| m2sockets | SocketsBridge | m2_shutdown | Shutdown a socket with given mode |
| m2sockets | SocketsBridge | m2_socket | Create a socket with given family and type |
| m2sockets | SocketsBridge | m2_strerror | Get error string for errno |
| m2stream | Stream | CloseAsync | Initiate graceful close asynchronously; returns a Future |
| m2stream | Stream | CreateTCP | Create a Stream over a connected non-blocking TCP socket |
| m2stream | Stream | CreateTLS | Create a Stream over a completed TLS session |
| m2stream | Stream | Destroy | Destroy the stream and release resources |
| m2stream | Stream | GetFd | Return the underlying file descriptor |
| m2stream | Stream | GetKind | Return the stream transport kind (TCP or TLS) |
| m2stream | Stream | GetState | Query the current stream state |
| m2stream | Stream | ReadAsync | Read up to max bytes asynchronously; returns a Future |
| m2stream | Stream | ShutdownWrite | Half-close the write side synchronously |
| m2stream | Stream | TryRead | Attempt to read up to max bytes (non-blocking, try-once) |
| m2stream | Stream | TryWrite | Attempt to write up to len bytes (non-blocking, try-once) |
| m2stream | Stream | WriteAllAsync | Write all bytes asynchronously (loops); returns a Future |
| m2stream | Stream | WriteAsync | Write up to len bytes asynchronously; returns a Future |
| m2sys | (C shim) | m2sys_basename | Extract base name from a path |
| m2sys | (C shim) | m2sys_chdir | Change current working directory |
| m2sys | (C shim) | m2sys_copy_file | Copy a file from src to dst |
| m2sys | (C shim) | m2sys_dirname | Extract directory name from a path |
| m2sys | (C shim) | m2sys_exec | Execute a shell command |
| m2sys | (C shim) | m2sys_exec_output | Execute a command and capture stdout into buffer |
| m2sys | (C shim) | m2sys_exit | Exit the process with given code |
| m2sys | (C shim) | m2sys_fclose | Close a file handle |
| m2sys | (C shim) | m2sys_file_exists | Check if a file exists |
| m2sys | (C shim) | m2sys_file_mtime | Get file modification time as Unix timestamp |
| m2sys | (C shim) | m2sys_file_size | Get file size in bytes |
| m2sys | (C shim) | m2sys_flock | Acquire an advisory file lock |
| m2sys | (C shim) | m2sys_fopen | Open a file with given path and mode |
| m2sys | (C shim) | m2sys_fread_bytes | Read raw bytes from a file handle |
| m2sys | (C shim) | m2sys_fread_line | Read a line from a file handle |
| m2sys | (C shim) | m2sys_funlock | Release an advisory file lock |
| m2sys | (C shim) | m2sys_fwrite_bytes | Write raw bytes to a file handle |
| m2sys | (C shim) | m2sys_fwrite_str | Write a string to a file handle |
| m2sys | (C shim) | m2sys_getcwd | Get current working directory |
| m2sys | (C shim) | m2sys_getenv | Get environment variable value |
| m2sys | (C shim) | m2sys_home_dir | Get user home directory |
| m2sys | (C shim) | m2sys_is_dir | Check if path is a directory |
| m2sys | (C shim) | m2sys_is_symlink | Check if path is a symbolic link |
| m2sys | (C shim) | m2sys_join_path | Join two path components |
| m2sys | (C shim) | m2sys_list_dir | List directory entries into a buffer |
| m2sys | (C shim) | m2sys_mkdir_p | Create directories recursively |
| m2sys | (C shim) | m2sys_remove_file | Remove a file |
| m2sys | (C shim) | m2sys_rename | Rename a file or directory |
| m2sys | (C shim) | m2sys_rmdir_r | Remove a directory recursively |
| m2sys | (C shim) | m2sys_sha256_file | Compute SHA-256 hash of a file |
| m2sys | (C shim) | m2sys_sha256_str | Compute SHA-256 hash of a string |
| m2sys | (C shim) | m2sys_str_append | Append src to dst buffer |
| m2sys | (C shim) | m2sys_str_contains_ci | Case-insensitive substring search |
| m2sys | (C shim) | m2sys_str_eq | Compare two strings for equality |
| m2sys | (C shim) | m2sys_str_starts_with | Check if string starts with prefix |
| m2sys | (C shim) | m2sys_strlen | Return string length |
| m2sys | (C shim) | m2sys_tar_create | Create a tar archive from a directory |
| m2sys | (C shim) | m2sys_tar_create_ex | Create a tar archive with exclude pattern |
| m2sys | (C shim) | m2sys_tar_extract | Extract a tar archive to a directory |
| m2sys | (C shim) | m2sys_unix_time | Get current Unix timestamp in seconds |
| m2text | Text | CountLines | Count lines in buffer (LF count + 1) |
| m2text | Text | DetectLineEnding | Detect line ending style (LF, CRLF, CR, Mixed) |
| m2text | Text | HasBOM | Returns 3 if buffer starts with UTF-8 BOM, 0 otherwise |
| m2text | Text | IsASCII | Return TRUE if every byte in buffer is < 128 |
| m2text | Text | IsBinary | Heuristic: return TRUE if buffer appears to be binary |
| m2text | Text | IsText | Heuristic: return TRUE if buffer appears to be text |
| m2text | Text | IsValidUTF8 | Full UTF-8 validation rejecting overlong and surrogates |
| m2text | Text | ParseShebang | Extract interpreter name from a #! line |
| m2tls | TLS | ContextCreate | Create a TLS client context with default settings |
| m2tls | TLS | ContextCreateServer | Create a TLS server context |
| m2tls | TLS | ContextDestroy | Destroy a TLS context |
| m2tls | TLS | GetALPN | Query the negotiated ALPN protocol after handshake |
| m2tls | TLS | GetLastError | Copy the last TLS engine error string into output |
| m2tls | TLS | GetPeerSummary | Copy the peer certificate subject into output |
| m2tls | TLS | GetVerifyResult | Return the X509 verification result code |
| m2tls | TLS | Handshake | Attempt one step of the TLS handshake (non-blocking) |
| m2tls | TLS | HandshakeAsync | Complete the TLS handshake asynchronously; returns a Future |
| m2tls | TLS | LoadCAFile | Load a CA bundle from a PEM file |
| m2tls | TLS | LoadSystemRoots | Load the system default CA root store |
| m2tls | TLS | Read | Attempt to read up to max bytes from TLS session |
| m2tls | TLS | ReadAsync | Read up to max bytes asynchronously; returns a Future |
| m2tls | TLS | SessionCreate | Create a TLS client session over a connected socket |
| m2tls | TLS | SessionCreateServer | Create a TLS server session over an accepted socket |
| m2tls | TLS | SessionDestroy | Destroy a TLS session (does not close socket) |
| m2tls | TLS | SetALPN | Set client-side ALPN protocol list |
| m2tls | TLS | SetALPNServer | Set server-side ALPN preferred protocol list |
| m2tls | TLS | SetClientCert | Load client certificate and private key from PEM files |
| m2tls | TLS | SetMinVersion | Set minimum acceptable TLS protocol version |
| m2tls | TLS | SetSNI | Set SNI hostname for the session |
| m2tls | TLS | SetServerCert | Load server certificate and private key from PEM files |
| m2tls | TLS | SetVerifyMode | Set peer certificate verification mode |
| m2tls | TLS | Shutdown | Initiate TLS shutdown (send close_notify) |
| m2tls | TLS | Write | Attempt to write up to len bytes to TLS session |
| m2tls | TLS | WriteAllAsync | Write all bytes asynchronously; returns a Future |
| m2tls | TLS | WriteAsync | Write up to len bytes asynchronously; returns a Future |
| m2tls | TlsBridge | m2_tls_ctx_create | Create a client TLS context (SSL_CTX) |
| m2tls | TlsBridge | m2_tls_ctx_create_server | Create a server TLS context |
| m2tls | TlsBridge | m2_tls_ctx_destroy | Destroy a TLS context |
| m2tls | TlsBridge | m2_tls_ctx_load_ca_file | Load CA bundle from PEM file |
| m2tls | TlsBridge | m2_tls_ctx_load_system_roots | Load system default CA root store |
| m2tls | TlsBridge | m2_tls_ctx_set_alpn | Set client ALPN protocol list |
| m2tls | TlsBridge | m2_tls_ctx_set_alpn_server | Set server ALPN protocol list |
| m2tls | TlsBridge | m2_tls_ctx_set_client_cert | Load client cert and key from PEM |
| m2tls | TlsBridge | m2_tls_ctx_set_min_version | Set minimum TLS protocol version |
| m2tls | TlsBridge | m2_tls_ctx_set_server_cert | Load server cert and key from PEM |
| m2tls | TlsBridge | m2_tls_ctx_set_verify | Set verification mode on context |
| m2tls | TlsBridge | m2_tls_get_alpn | Get negotiated ALPN protocol string |
| m2tls | TlsBridge | m2_tls_get_last_error | Get last TLS error string |
| m2tls | TlsBridge | m2_tls_get_peer_summary | Get peer certificate subject summary |
| m2tls | TlsBridge | m2_tls_get_verify_result | Get X509 verification result code |
| m2tls | TlsBridge | m2_tls_handshake | Perform one step of TLS handshake |
| m2tls | TlsBridge | m2_tls_init | Initialize TLS/OpenSSL library |
| m2tls | TlsBridge | m2_tls_read | Read decrypted bytes from TLS session |
| m2tls | TlsBridge | m2_tls_session_create | Create a client TLS session on fd |
| m2tls | TlsBridge | m2_tls_session_create_server | Create a server TLS session on fd |
| m2tls | TlsBridge | m2_tls_session_destroy | Destroy a TLS session |
| m2tls | TlsBridge | m2_tls_session_set_sni | Set SNI hostname on session |
| m2tls | TlsBridge | m2_tls_shutdown | Send TLS close_notify |
| m2tls | TlsBridge | m2_tls_write | Write bytes through TLS session |
