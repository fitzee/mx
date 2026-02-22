# Flow Control

## Overview

HTTP/2 uses two levels of flow control (RFC 7540 Section 5.2):

1. **Connection-level**: shared across all streams
2. **Per-stream**: independent for each stream

Both start at 65535 bytes (default initial window size).

## Receive Side (client → server)

When the server receives DATA frames:

1. Check payload size against connection receive window
2. If exceeded → GOAWAY with FLOW_CONTROL_ERROR
3. Deduct from connection window
4. Send WINDOW_UPDATE on stream 0 (connection) to replenish
5. Send WINDOW_UPDATE on stream ID to replenish
6. Accumulate data in request body

The server immediately replenishes windows after consuming data,
preventing backpressure from blocking clients.

## Send Side (server → client)

When sending response DATA frames:

1. Calculate sendable = min(remaining, maxFrameSize, connWindow, streamWindow)
2. If sendable = 0 → stop; caller must retry after WINDOW_UPDATE
3. Consume from stream send window via `ConsumeSendWindow`
4. Deduct from connection send window
5. Encode DATA frame header + payload
6. Advance body offset

If the send window is exhausted mid-response, `SendResponse` returns
early. The caller must call `FlushData` after receiving WINDOW_UPDATE
frames from the client.

## WINDOW_UPDATE Handling

Incoming WINDOW_UPDATE frames:
- Stream 0: updates `connRecvWindow` (connection level)
- Stream N: calls `UpdateSendWindow` on the stream's H2Stream

Zero-increment WINDOW_UPDATE is a protocol error.

## Settings

The `SETTINGS_INITIAL_WINDOW_SIZE` parameter affects new streams.
When the remote peer changes this setting, existing streams are NOT
retroactively adjusted (simplified implementation).
