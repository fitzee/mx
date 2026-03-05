/* m2sys_test.c — standalone test for m2sys C library functions */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>
#include <sys/stat.h>
#include "../libs/m2sys/m2sys.h"

static int tests_passed = 0;
static int tests_failed = 0;

#define CHECK(cond, msg) do { \
    if (cond) { tests_passed++; } \
    else { tests_failed++; fprintf(stderr, "FAIL: %s (line %d)\n", msg, __LINE__); } \
} while(0)

static char tmpdir[256];

static void make_tmpdir(void) {
    snprintf(tmpdir, sizeof(tmpdir), "/tmp/m2sys_test_%d", (int)getpid());
    m2sys_mkdir_p(tmpdir);
}

static void test_file_io(void) {
    char path[512], mode[4], data[256], line[256];
    int32_t fh, rc;

    snprintf(path, sizeof(path), "%s/test.txt", tmpdir);
    strcpy(mode, "w");
    fh = m2sys_fopen(path, mode);
    CHECK(fh >= 0, "fopen for write");

    strcpy(data, "hello world\n");
    rc = m2sys_fwrite_str(fh, data);
    CHECK(rc == 0, "fwrite_str");

    strcpy(data, "line two\n");
    rc = m2sys_fwrite_str(fh, data);
    CHECK(rc == 0, "fwrite_str line 2");

    rc = m2sys_fclose(fh);
    CHECK(rc == 0, "fclose write");

    /* Read back */
    strcpy(mode, "r");
    fh = m2sys_fopen(path, mode);
    CHECK(fh >= 0, "fopen for read");

    rc = m2sys_fread_line(fh, line, sizeof(line));
    CHECK(rc >= 0, "fread_line 1");
    CHECK(strcmp(line, "hello world") == 0, "line 1 content");

    rc = m2sys_fread_line(fh, line, sizeof(line));
    CHECK(rc >= 0, "fread_line 2");
    CHECK(strcmp(line, "line two") == 0, "line 2 content");

    rc = m2sys_fread_line(fh, line, sizeof(line));
    CHECK(rc < 0, "fread_line EOF");

    rc = m2sys_fclose(fh);
    CHECK(rc == 0, "fclose read");
}

static void test_file_exists_is_dir(void) {
    char path[512];
    snprintf(path, sizeof(path), "%s/test.txt", tmpdir);
    CHECK(m2sys_file_exists(path) == 1, "file_exists true");
    CHECK(m2sys_file_exists("/nonexistent_xyz") == 0, "file_exists false");
    CHECK(m2sys_is_dir(tmpdir) == 1, "is_dir true");
    CHECK(m2sys_is_dir(path) == 0, "is_dir false on file");
}

static void test_mkdir_p(void) {
    char path[512];
    snprintf(path, sizeof(path), "%s/a/b/c", tmpdir);
    int32_t rc = m2sys_mkdir_p(path);
    CHECK(rc == 0, "mkdir_p");
    CHECK(m2sys_is_dir(path) == 1, "mkdir_p created dirs");
}

static void test_sha256_str(void) {
    char hex[65];
    /* SHA-256 of empty string */
    m2sys_sha256_str("", 0, hex);
    CHECK(strcmp(hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855") == 0,
          "sha256 empty string");

    m2sys_sha256_str("hello", 5, hex);
    CHECK(strcmp(hex, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824") == 0,
          "sha256 'hello'");
}

static void test_sha256_file(void) {
    char path[512], hex[65];
    snprintf(path, sizeof(path), "%s/test.txt", tmpdir);
    int32_t rc = m2sys_sha256_file(path, hex);
    CHECK(rc == 0, "sha256_file success");
    CHECK(strlen(hex) == 64, "sha256_file hex length");
}

static void test_file_size(void) {
    char path[512];
    snprintf(path, sizeof(path), "%s/test.txt", tmpdir);
    int64_t sz = m2sys_file_size(path);
    CHECK(sz > 0, "file_size > 0");
    CHECK(m2sys_file_size("/nonexistent_xyz") == -1, "file_size nonexistent");
}

static void test_copy_file(void) {
    char src[512], dst[512];
    snprintf(src, sizeof(src), "%s/test.txt", tmpdir);
    snprintf(dst, sizeof(dst), "%s/test_copy.txt", tmpdir);
    int32_t rc = m2sys_copy_file(src, dst);
    CHECK(rc == 0, "copy_file");
    CHECK(m2sys_file_exists(dst) == 1, "copy_file dest exists");
    CHECK(m2sys_file_size(src) == m2sys_file_size(dst), "copy_file same size");
}

static void test_rename(void) {
    char src[512], dst[512];
    snprintf(src, sizeof(src), "%s/test_copy.txt", tmpdir);
    snprintf(dst, sizeof(dst), "%s/test_renamed.txt", tmpdir);
    int32_t rc = m2sys_rename(src, dst);
    CHECK(rc == 0, "rename");
    CHECK(m2sys_file_exists(dst) == 1, "rename dest exists");
    CHECK(m2sys_file_exists(src) == 0, "rename src gone");
}

static void test_list_dir(void) {
    char buf[4096];
    int32_t n = m2sys_list_dir(tmpdir, buf, sizeof(buf));
    CHECK(n > 0, "list_dir non-empty");
    CHECK(strstr(buf, "test.txt") != NULL, "list_dir contains test.txt");
}

static void test_basename_dirname(void) {
    char out[256];
    m2sys_basename("/foo/bar/baz.txt", out, sizeof(out));
    CHECK(strcmp(out, "baz.txt") == 0, "basename");

    m2sys_dirname("/foo/bar/baz.txt", out, sizeof(out));
    CHECK(strcmp(out, "/foo/bar") == 0, "dirname");
}

static void test_exec_output(void) {
    char buf[256];
    int32_t rc = m2sys_exec_output("echo hello_exec", buf, sizeof(buf));
    CHECK(rc == 0, "exec_output exit code");
    /* Trim trailing newline for comparison */
    int len = (int)strlen(buf);
    if (len > 0 && buf[len-1] == '\n') buf[len-1] = '\0';
    CHECK(strcmp(buf, "hello_exec") == 0, "exec_output content");
}

static void test_join_path(void) {
    char out[256];
    m2sys_join_path("/foo", "bar", out, sizeof(out));
    CHECK(strcmp(out, "/foo/bar") == 0, "join_path");

    m2sys_join_path("/foo/", "bar", out, sizeof(out));
    CHECK(strcmp(out, "/foo/bar") == 0, "join_path trailing slash");
}

static void test_home_dir(void) {
    char out[256];
    m2sys_home_dir(out, sizeof(out));
    CHECK(strlen(out) > 0, "home_dir non-empty");
}

static void test_getenv(void) {
    char out[256];
    m2sys_getenv("HOME", out, sizeof(out));
    CHECK(strlen(out) > 0, "getenv HOME");

    m2sys_getenv("M2SYS_NONEXISTENT_VAR_XYZ", out, sizeof(out));
    CHECK(strlen(out) == 0, "getenv nonexistent");
}

static void test_getcwd_chdir(void) {
    char orig[512], check[512];
    m2sys_getcwd(orig, sizeof(orig));
    CHECK(strlen(orig) > 0, "getcwd");

    int32_t rc = m2sys_chdir(tmpdir);
    CHECK(rc == 0, "chdir");

    m2sys_getcwd(check, sizeof(check));
    /* On macOS /tmp is a symlink to /private/tmp */
    CHECK(strstr(check, "m2sys_test") != NULL, "chdir verified");

    m2sys_chdir(orig);
}

static void test_remove_file(void) {
    char path[512];
    snprintf(path, sizeof(path), "%s/test_renamed.txt", tmpdir);
    CHECK(m2sys_file_exists(path) == 1, "remove target exists");
    int32_t rc = m2sys_remove_file(path);
    CHECK(rc == 0, "remove_file");
    CHECK(m2sys_file_exists(path) == 0, "remove_file verified");
}

static void test_flock(void) {
    char path[512], mode[4];
    snprintf(path, sizeof(path), "%s/lock_test.txt", tmpdir);
    strcpy(mode, "w");
    int32_t fh = m2sys_fopen(path, mode);
    CHECK(fh >= 0, "flock: open file");

    int32_t rc = m2sys_flock(fh, 1);
    CHECK(rc == 0, "flock exclusive");

    rc = m2sys_funlock(fh);
    CHECK(rc == 0, "funlock");

    m2sys_fclose(fh);
}

static void test_strlen(void) {
    CHECK(m2sys_strlen("hello") == 5, "strlen");
    CHECK(m2sys_strlen("") == 0, "strlen empty");
}

static void test_tar_create_ex(void) {
    char srcdir[512], tarpath[512], destdir[512], fpath[512], mode[4];
    int32_t fh, rc;

    /* Create a source directory with files and an excluded subdir */
    snprintf(srcdir, sizeof(srcdir), "%s/tarsrc", tmpdir);
    m2sys_mkdir_p(srcdir);

    snprintf(fpath, sizeof(fpath), "%s/main.mod", srcdir);
    strcpy(mode, "w");
    fh = m2sys_fopen(fpath, mode);
    m2sys_fwrite_str(fh, "MODULE Main; END Main.");
    m2sys_fclose(fh);

    char excldir[512];
    snprintf(excldir, sizeof(excldir), "%s/target", srcdir);
    m2sys_mkdir_p(excldir);
    snprintf(fpath, sizeof(fpath), "%s/output.o", excldir);
    fh = m2sys_fopen(fpath, mode);
    m2sys_fwrite_str(fh, "binary");
    m2sys_fclose(fh);

    /* Create tar with --exclude target */
    snprintf(tarpath, sizeof(tarpath), "%s/test.tar", tmpdir);
    rc = m2sys_tar_create_ex(tarpath, srcdir, "target");
    CHECK(rc == 0, "tar_create_ex");
    CHECK(m2sys_file_exists(tarpath) == 1, "tar_create_ex: tarball exists");

    /* Extract and verify target/ was excluded */
    snprintf(destdir, sizeof(destdir), "%s/tarout", tmpdir);
    rc = m2sys_tar_extract(tarpath, destdir);
    CHECK(rc == 0, "tar_create_ex: extract");

    snprintf(fpath, sizeof(fpath), "%s/main.mod", destdir);
    CHECK(m2sys_file_exists(fpath) == 1, "tar_create_ex: main.mod included");

    snprintf(fpath, sizeof(fpath), "%s/target", destdir);
    CHECK(m2sys_is_dir(fpath) == 0, "tar_create_ex: target/ excluded");

    /* Cleanup */
    m2sys_rmdir_r(srcdir);
    m2sys_rmdir_r(destdir);
    m2sys_remove_file(tarpath);
}

static void test_rmdir_r(void) {
    char path[512];
    snprintf(path, sizeof(path), "%s/rmtest", tmpdir);
    m2sys_mkdir_p(path);
    CHECK(m2sys_is_dir(path) == 1, "rmdir_r: dir exists");
    /* Create a file inside */
    char fpath[512], mode[4];
    snprintf(fpath, sizeof(fpath), "%s/file.txt", path);
    strcpy(mode, "w");
    int32_t fh = m2sys_fopen(fpath, mode);
    m2sys_fwrite_str(fh, "test");
    m2sys_fclose(fh);
    /* Remove recursively */
    int32_t rc = m2sys_rmdir_r(path);
    CHECK(rc == 0, "rmdir_r");
    CHECK(m2sys_is_dir(path) == 0, "rmdir_r: dir gone");
}

int main(void) {
    make_tmpdir();

    test_file_io();
    test_file_exists_is_dir();
    test_mkdir_p();
    test_sha256_str();
    test_sha256_file();
    test_file_size();
    test_copy_file();
    test_rename();
    test_list_dir();
    test_basename_dirname();
    test_exec_output();
    test_join_path();
    test_home_dir();
    test_getenv();
    test_getcwd_chdir();
    test_remove_file();
    test_flock();
    test_strlen();
    test_tar_create_ex();
    test_rmdir_r();

    /* Cleanup */
    m2sys_rmdir_r(tmpdir);

    printf("\nm2sys_test: %d passed, %d failed\n", tests_passed, tests_failed);
    return tests_failed > 0 ? 1 : 0;
}
