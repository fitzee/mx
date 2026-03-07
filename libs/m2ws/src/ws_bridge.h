/*
 * ws_bridge.h -- WebSocket C helpers for m2ws
 *
 * Provides:
 *   - SHA-1 hash (for Sec-WebSocket-Accept computation)
 *   - Base64 encoding
 *   - XOR mask application (for frame payload masking)
 *   - Pseudo-random mask key generation
 *
 * The M2 layer (WsFrame.mod, WebSocket.mod) owns all higher-level logic.
 */

#ifndef WS_BRIDGE_H
#define WS_BRIDGE_H

#include <stdint.h>

/* ── SHA-1 ──────────────────────────────────────────────────────────── */

/* Compute SHA-1 hash of data[0..len-1].
   out20 must point to at least 20 bytes. */
void m2_ws_sha1(const void *data, int32_t len, void *out20);

/* ── Base64 ─────────────────────────────────────────────────────────── */

/* Base64-encode in[0..inLen-1] into out.
   maxOut is the capacity of out (must be >= ((inLen+2)/3)*4 + 1).
   *outLen receives the number of characters written (not counting NUL). */
void m2_ws_base64_encode(const void *in, int32_t inLen,
                         void *out, int32_t maxOut,
                         int32_t *outLen);

/* ── XOR mask ───────────────────────────────────────────────────────── */

/* Apply XOR mask in-place: data[i] ^= mask[(offset+i) % 4].
   mask must point to 4 bytes. */
void m2_ws_apply_mask(void *data, int32_t len,
                      const void *mask, int32_t offset);

/* ── Random mask key ────────────────────────────────────────────────── */

/* Write 4 pseudo-random bytes to out.
   Not cryptographic quality -- sufficient for WebSocket masking. */
void m2_ws_random_mask(void *out);

#endif /* WS_BRIDGE_H */
