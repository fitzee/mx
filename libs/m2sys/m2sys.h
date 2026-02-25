#ifndef M2SYS_H
#define M2SYS_H

#include <stdint.h>

/* File I/O — handle-based, small table */
int32_t m2sys_fopen(void *path, void *mode);
int32_t m2sys_fclose(int32_t handle);
int32_t m2sys_fread_line(int32_t handle, void *buf, int32_t buf_size);
int32_t m2sys_fwrite_str(int32_t handle, void *data);

/* File system */
int32_t m2sys_file_exists(void *path);
int32_t m2sys_is_dir(void *path);
int32_t m2sys_mkdir_p(void *path);
int32_t m2sys_remove_file(void *path);

/* Process execution */
int32_t m2sys_exec(void *cmdline);
void    m2sys_exit(int32_t code);

/* SHA-256 */
void m2sys_sha256_str(void *data, int32_t len, void *hex_out);

/* Paths & environment */
void m2sys_join_path(void *a, void *b, void *out, int32_t out_size);
void m2sys_home_dir(void *out, int32_t out_size);
void m2sys_getcwd(void *out, int32_t out_size);
int32_t m2sys_chdir(void *path);
void m2sys_getenv(void *name, void *out, int32_t out_size);

/* String utilities */
int32_t m2sys_strlen(void *s);
int32_t m2sys_str_eq(const void *a, const void *b);
int32_t m2sys_str_starts_with(const void *s, const void *prefix);
int32_t m2sys_str_append(void *dst, int32_t dst_size, const void *src);
int32_t m2sys_str_contains_ci(const void *haystack, const void *needle);

/* File operations */
int32_t m2sys_sha256_file(void *path, void *hex_out);
int32_t m2sys_file_size(void *path);
int32_t m2sys_copy_file(void *src, void *dst);
int32_t m2sys_rename(void *old_path, void *new_path);

/* Directory listing */
int32_t m2sys_list_dir(void *dir, void *buf, int32_t bufSize);

/* Path utilities */
void m2sys_basename(void *path, void *out, int32_t outSize);
void m2sys_dirname(void *path, void *out, int32_t outSize);

/* Process execution (captures stdout) */
int32_t m2sys_exec_output(void *cmdline, void *outBuf, int32_t outSize);

/* Tar (POSIX ustar, no compression) */
int32_t m2sys_tar_create(void *archivePath, void *baseDir);
int32_t m2sys_tar_create_ex(void *archivePath, void *baseDir, void *excludePattern);
int32_t m2sys_tar_extract(void *archivePath, void *destDir);

/* Binary file I/O */
int32_t m2sys_fwrite_bytes(int32_t handle, const void *data, int32_t len);
int32_t m2sys_fread_bytes(int32_t handle, void *buf, int32_t maxLen);

/* File locking (advisory, POSIX flock) */
int32_t m2sys_flock(int32_t handle, int32_t exclusive);
int32_t m2sys_funlock(int32_t handle);

/* Remove directory recursively */
int32_t m2sys_rmdir_r(void *path);

/* Time */
int64_t m2sys_unix_time(void);

/* File metadata */
int64_t m2sys_file_mtime(void *path);
int32_t m2sys_is_symlink(void *path);

#endif
