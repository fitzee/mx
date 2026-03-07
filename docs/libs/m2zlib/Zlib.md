# Zlib

## Why
Provides compression and decompression via the system zlib library. Supports Raw deflate, Zlib, and Gzip formats in both streaming and one-shot modes. Caller provides all buffers, so there is no hidden allocation on the Modula-2 side. Requires `-lz` at link time.

## Types

- **Stream** (ADDRESS) -- Opaque zlib stream handle.
- **Status** -- `Ok`, `StreamEnd`, `NeedMore`, `Error`.
- **Level** -- `NoCompression`, `BestSpeed`, `Default`, `BestCompression`.
- **Format** -- `Raw`, `ZlibFmt`, `Gzip`.

## Procedures

### Streaming Compression

- `PROCEDURE DeflateInit(VAR s: Stream; level: Level; fmt: Format): Status`
  Initialise a compression stream.

- `PROCEDURE Deflate(VAR s: Stream; src: ADDRESS; srcLen: CARDINAL; dst: ADDRESS; dstMax: CARDINAL; VAR produced: CARDINAL; flush: BOOLEAN): Status`
  Compress from src into dst. Set flush to TRUE to finalise the stream. produced is set to the number of bytes written to dst.

- `PROCEDURE DeflateEnd(VAR s: Stream): Status`
  Free compression stream resources.

### Streaming Decompression

- `PROCEDURE InflateInit(VAR s: Stream; fmt: Format): Status`
  Initialise a decompression stream.

- `PROCEDURE Inflate(VAR s: Stream; src: ADDRESS; srcLen: CARDINAL; dst: ADDRESS; dstMax: CARDINAL; VAR produced: CARDINAL): Status`
  Decompress from src into dst. produced is set to the number of bytes written to dst.

- `PROCEDURE InflateEnd(VAR s: Stream): Status`
  Free decompression stream resources.

### One-Shot Convenience

- `PROCEDURE Compress(src: ADDRESS; srcLen: CARDINAL; dst: ADDRESS; dstMax: CARDINAL; VAR dstLen: CARDINAL; fmt: Format): Status`
  Compress an entire buffer in one call.

- `PROCEDURE Decompress(src: ADDRESS; srcLen: CARDINAL; dst: ADDRESS; dstMax: CARDINAL; VAR dstLen: CARDINAL; fmt: Format): Status`
  Decompress an entire buffer in one call.

## Example

```modula2
MODULE ZlibDemo;

FROM SYSTEM IMPORT ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM Zlib IMPORT Status, Level, Format, Ok, Default, Gzip,
                 Compress, Decompress;

VAR
  src: ARRAY [0..63] OF CHAR;
  compressed: ARRAY [0..255] OF CHAR;
  decompressed: ARRAY [0..255] OF CHAR;
  compLen, decompLen: CARDINAL;
  s: Status;

BEGIN
  src := "Hello, compressed world!";

  s := Compress(ADR(src), 24, ADR(compressed), 256, compLen, Gzip);
  IF s = Ok THEN
    WriteString("compressed to ");
    WriteCard(compLen, 0);
    WriteString(" bytes"); WriteLn;

    s := Decompress(ADR(compressed), compLen,
                    ADR(decompressed), 256, decompLen, Gzip);
    IF s = Ok THEN
      WriteString(decompressed); WriteLn
    END
  END
END ZlibDemo.
```
