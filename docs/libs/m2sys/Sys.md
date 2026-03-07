# Sys

C FFI shim providing file I/O, filesystem operations, process execution, SHA-256 hashing, path utilities, tar archives, and time functions. This is the low-level system interface that higher-level Modula-2 libraries build on.

## Why Sys?

Modula-2 has no built-in file system access, process spawning, or cryptographic hashing. Sys bridges that gap with a flat C API (`m2sys.c` / `m2sys.h`) that Modula-2 code imports via a `DEFINITION MODULE FOR "C"` foreign binding. All functions use `ADDRESS`-based string passing (null-terminated) and integer return codes, making them directly callable from generated C code.

Sys is used by m2pkg, m2log, the build system, and any application that needs OS interaction.

## Linking

Compile and link `libs/m2sys/m2sys.c` with your program. The header is at `libs/m2sys/m2sys.h`.

```
m2c compile main.mod --extra-c libs/m2sys/m2sys.c -o main
```

## File I/O

Handle-based API with a fixed table of 32 open files.

### m2sys_fopen / m2sys_fclose

```c
int32_t m2sys_fopen(void *path, void *mode);   /* returns handle, or -1 */
int32_t m2sys_fclose(int32_t handle);           /* returns 0 on success */
```

Open a file with the given mode (`"r"`, `"w"`, `"a"`, etc.). Returns a handle (0..31) on success, -1 on failure. Close releases the handle.

### m2sys_fread_line

```c
int32_t m2sys_fread_line(int32_t handle, void *buf, int32_t buf_size);
```

Read one line into `buf` (including newline). Returns bytes read, or 0 at EOF, or -1 on error.

### m2sys_fwrite_str

```c
int32_t m2sys_fwrite_str(int32_t handle, void *data);
```

Write a null-terminated string. Returns 0 on success.

### m2sys_fread_bytes / m2sys_fwrite_bytes

```c
int32_t m2sys_fread_bytes(int32_t handle, void *buf, int32_t maxLen);
int32_t m2sys_fwrite_bytes(int32_t handle, const void *data, int32_t len);
```

Binary I/O. `fread_bytes` returns bytes read (0 at EOF). `fwrite_bytes` returns 0 on success.

## Filesystem

### m2sys_file_exists / m2sys_is_dir

```c
int32_t m2sys_file_exists(void *path);  /* 1=exists, 0=not */
int32_t m2sys_is_dir(void *path);       /* 1=directory, 0=not */
```

### m2sys_mkdir_p

```c
int32_t m2sys_mkdir_p(void *path);  /* 0=success, -1=error */
```

Create directory and all parent directories (like `mkdir -p`).

### m2sys_remove_file / m2sys_rmdir_r

```c
int32_t m2sys_remove_file(void *path);  /* 0=success */
int32_t m2sys_rmdir_r(void *path);      /* 0=success, recursive delete */
```

### m2sys_file_size / m2sys_copy_file / m2sys_rename

```c
int32_t m2sys_file_size(void *path);                    /* bytes, or -1 */
int32_t m2sys_copy_file(void *src, void *dst);          /* 0=success */
int32_t m2sys_rename(void *old_path, void *new_path);   /* 0=success */
```

### m2sys_list_dir

```c
int32_t m2sys_list_dir(void *dir, void *buf, int32_t bufSize);
```

List directory entries into `buf` as newline-separated names. Returns the number of entries, or -1 on error.

### m2sys_is_symlink

```c
int32_t m2sys_is_symlink(void *path);  /* 1=symlink, 0=not */
```

## Process Execution

### m2sys_exec

```c
int32_t m2sys_exec(void *cmdline);  /* returns exit code */
```

Execute a shell command. Returns the process exit code.

### m2sys_exec_output

```c
int32_t m2sys_exec_output(void *cmdline, void *outBuf, int32_t outSize);
```

Execute a command and capture its stdout into `outBuf`. Returns the exit code.

### m2sys_exit

```c
void m2sys_exit(int32_t code);
```

Terminate the process with the given exit code.

## SHA-256

### m2sys_sha256_str

```c
void m2sys_sha256_str(void *data, int32_t len, void *hex_out);
```

Compute SHA-256 of `len` bytes at `data`. Writes a 64-character lowercase hex string to `hex_out` (must be at least 65 bytes for the null terminator). Uses a public-domain implementation -- no external library dependency.

### m2sys_sha256_file

```c
int32_t m2sys_sha256_file(void *path, void *hex_out);  /* 0=success */
```

Hash a file's contents. Reads in 4 KB chunks.

## Paths & Environment

### m2sys_join_path

```c
void m2sys_join_path(void *a, void *b, void *out, int32_t out_size);
```

Join two path components with `/`.

### m2sys_home_dir / m2sys_getcwd / m2sys_chdir

```c
void m2sys_home_dir(void *out, int32_t out_size);
void m2sys_getcwd(void *out, int32_t out_size);
int32_t m2sys_chdir(void *path);  /* 0=success */
```

### m2sys_getenv

```c
void m2sys_getenv(void *name, void *out, int32_t out_size);
```

Get an environment variable. Writes empty string if not set.

### m2sys_basename / m2sys_dirname

```c
void m2sys_basename(void *path, void *out, int32_t outSize);
void m2sys_dirname(void *path, void *out, int32_t outSize);
```

## String Utilities

```c
int32_t m2sys_strlen(void *s);
int32_t m2sys_str_eq(const void *a, const void *b);              /* 1=equal */
int32_t m2sys_str_starts_with(const void *s, const void *prefix); /* 1=yes */
int32_t m2sys_str_append(void *dst, int32_t dst_size, const void *src);
int32_t m2sys_str_contains_ci(const void *haystack, const void *needle); /* 1=found */
```

`str_contains_ci` performs case-insensitive substring search (ASCII only).

## Tar Archives

```c
int32_t m2sys_tar_create(void *archivePath, void *baseDir);
int32_t m2sys_tar_create_ex(void *archivePath, void *baseDir, void *excludePattern);
int32_t m2sys_tar_extract(void *archivePath, void *destDir);
```

Create and extract tar archives. Shells out to the system `tar` command. `tar_create_ex` accepts an exclude pattern (e.g., `".git"`). Returns 0 on success.

## File Locking

```c
int32_t m2sys_flock(int32_t handle, int32_t exclusive);  /* 0=success */
int32_t m2sys_funlock(int32_t handle);                    /* 0=success */
```

Advisory POSIX file locking. Pass `exclusive=1` for exclusive lock, `0` for shared.

## Time

```c
int64_t m2sys_unix_time(void);           /* seconds since epoch */
int64_t m2sys_file_mtime(void *path);    /* file modification time */
```

## Example (from Modula-2)

```modula2
DEFINITION MODULE FOR "C" Sys;
FROM SYSTEM IMPORT ADDRESS;
PROCEDURE m2sys_file_exists(path: ADDRESS): INTEGER;
PROCEDURE m2sys_exec(cmdline: ADDRESS): INTEGER;
PROCEDURE m2sys_sha256_str(data: ADDRESS; len: INTEGER; hexOut: ADDRESS);
END Sys.
```

```modula2
FROM Sys IMPORT m2sys_file_exists, m2sys_exec;
FROM SYSTEM IMPORT ADR;

VAR path: ARRAY [0..255] OF CHAR;
BEGIN
  path := "build/output";
  IF m2sys_file_exists(ADR(path)) = 1 THEN
    m2sys_exec(ADR("rm -rf build/output"))
  END
END
```
