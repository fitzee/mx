#include "m2sys.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <errno.h>

/* ── File handle table ─────────────────────────────────────────── */

#define MAX_HANDLES 32
static FILE *handle_table[MAX_HANDLES];

static int alloc_handle(FILE *fp) {
    for (int i = 0; i < MAX_HANDLES; i++) {
        if (!handle_table[i]) { handle_table[i] = fp; return i; }
    }
    return -1;
}

int32_t m2sys_fopen(void *path, void *mode) {
    FILE *fp = fopen((const char *)path, (const char *)mode);
    if (!fp) return -1;
    return (int32_t)alloc_handle(fp);
}

int32_t m2sys_fclose(int32_t handle) {
    if (handle < 0 || handle >= MAX_HANDLES || !handle_table[handle]) return -1;
    fclose(handle_table[handle]);
    handle_table[handle] = NULL;
    return 0;
}

int32_t m2sys_fread_line(int32_t handle, void *buf, int32_t buf_size) {
    if (handle < 0 || handle >= MAX_HANDLES || !handle_table[handle]) return -1;
    char *s = fgets((char *)buf, buf_size, handle_table[handle]);
    if (!s) return -1;
    /* Strip trailing newline */
    int32_t len = (int32_t)strlen(s);
    if (len > 0 && s[len - 1] == '\n') { s[--len] = '\0'; }
    if (len > 0 && s[len - 1] == '\r') { s[--len] = '\0'; }
    return len;
}

int32_t m2sys_fwrite_str(int32_t handle, void *data) {
    if (handle < 0 || handle >= MAX_HANDLES || !handle_table[handle]) return -1;
    int32_t n = (int32_t)fputs((const char *)data, handle_table[handle]);
    return n >= 0 ? 0 : -1;
}

/* ── Binary file I/O ───────────────────────────────────────────── */

int32_t m2sys_fwrite_bytes(int32_t handle, const void *data, int32_t len) {
    if (handle < 0 || handle >= MAX_HANDLES || !handle_table[handle]) return -1;
    if (len <= 0) return 0;
    size_t written = fwrite(data, 1, (size_t)len, handle_table[handle]);
    return (int32_t)written;
}

int32_t m2sys_fread_bytes(int32_t handle, void *buf, int32_t maxLen) {
    if (handle < 0 || handle >= MAX_HANDLES || !handle_table[handle]) return -1;
    if (maxLen <= 0) return 0;
    size_t rd = fread(buf, 1, (size_t)maxLen, handle_table[handle]);
    if (rd == 0 && ferror(handle_table[handle])) return -1;
    return (int32_t)rd;
}

/* ── Filesystem ────────────────────────────────────────────────── */

int32_t m2sys_file_exists(void *path) {
    return access((const char *)path, F_OK) == 0 ? 1 : 0;
}

int32_t m2sys_is_dir(void *path) {
    struct stat st;
    if (stat((const char *)path, &st) != 0) return 0;
    return S_ISDIR(st.st_mode) ? 1 : 0;
}

/* Recursive mkdir -p */
int32_t m2sys_mkdir_p(void *path) {
    char tmp[1024];
    strncpy(tmp, (const char *)path, sizeof(tmp) - 1);
    tmp[sizeof(tmp) - 1] = '\0';
    size_t len = strlen(tmp);
    if (len > 0 && tmp[len - 1] == '/') tmp[len - 1] = '\0';
    for (char *p = tmp + 1; *p; p++) {
        if (*p == '/') {
            *p = '\0';
            mkdir(tmp, 0755);
            *p = '/';
        }
    }
    return mkdir(tmp, 0755) == 0 || errno == EEXIST ? 0 : -1;
}

int32_t m2sys_remove_file(void *path) {
    return remove((const char *)path) == 0 ? 0 : -1;
}

/* ── Process execution ─────────────────────────────────────────── */

int32_t m2sys_exec(void *cmdline) {
    int rc = system((const char *)cmdline);
    /* POSIX: system() returns the exit status from waitpid.
       Extract the actual exit code. */
    if (rc == -1) return -1;
#ifdef _WIN32
    return (int32_t)rc;
#else
    return WIFEXITED(rc) ? (int32_t)WEXITSTATUS(rc) : -1;
#endif
}

void m2sys_exit(int32_t code) {
    exit(code);
}

/* ── SHA-256 (minimal, public-domain implementation) ───────────── */

static uint32_t sha_k[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

#define RR(x,n) (((x)>>(n))|((x)<<(32-(n))))
#define CH(x,y,z) (((x)&(y))^((~(x))&(z)))
#define MAJ(x,y,z) (((x)&(y))^((x)&(z))^((y)&(z)))
#define EP0(x) (RR(x,2)^RR(x,13)^RR(x,22))
#define EP1(x) (RR(x,6)^RR(x,11)^RR(x,25))
#define SIG0(x) (RR(x,7)^RR(x,18)^((x)>>3))
#define SIG1(x) (RR(x,17)^RR(x,19)^((x)>>10))

static void sha256_transform(uint32_t state[8], const uint8_t block[64]) {
    uint32_t w[64], a, b, c, d, e, f, g, h, t1, t2;
    for (int i = 0; i < 16; i++)
        w[i] = ((uint32_t)block[i*4]<<24)|((uint32_t)block[i*4+1]<<16)|
               ((uint32_t)block[i*4+2]<<8)|(uint32_t)block[i*4+3];
    for (int i = 16; i < 64; i++)
        w[i] = SIG1(w[i-2]) + w[i-7] + SIG0(w[i-15]) + w[i-16];
    a=state[0]; b=state[1]; c=state[2]; d=state[3];
    e=state[4]; f=state[5]; g=state[6]; h=state[7];
    for (int i = 0; i < 64; i++) {
        t1 = h + EP1(e) + CH(e,f,g) + sha_k[i] + w[i];
        t2 = EP0(a) + MAJ(a,b,c);
        h=g; g=f; f=e; e=d+t1; d=c; c=b; b=a; a=t1+t2;
    }
    state[0]+=a; state[1]+=b; state[2]+=c; state[3]+=d;
    state[4]+=e; state[5]+=f; state[6]+=g; state[7]+=h;
}

void m2sys_sha256_str(void *data, int32_t len, void *hex_out) {
    const uint8_t *msg = (const uint8_t *)data;
    uint32_t state[8] = {
        0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
        0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19
    };
    uint64_t bitlen = (uint64_t)len * 8;
    uint8_t block[64];
    int32_t i = 0;
    /* Process full blocks */
    for (; i + 64 <= len; i += 64)
        sha256_transform(state, msg + i);
    /* Padding */
    int rem = len - i;
    memset(block, 0, 64);
    if (rem > 0) memcpy(block, msg + i, rem);
    block[rem] = 0x80;
    if (rem >= 56) {
        sha256_transform(state, block);
        memset(block, 0, 64);
    }
    for (int j = 0; j < 8; j++)
        block[63 - j] = (uint8_t)(bitlen >> (j * 8));
    sha256_transform(state, block);
    /* Output hex */
    char *out = (char *)hex_out;
    for (int j = 0; j < 8; j++)
        sprintf(out + j * 8, "%08x", state[j]);
    out[64] = '\0';
}

/* ── Paths & environment ───────────────────────────────────────── */

void m2sys_join_path(void *a, void *b, void *out, int32_t out_size) {
    const char *sa = (const char *)a;
    const char *sb = (const char *)b;
    char *so = (char *)out;
    int la = (int)strlen(sa);
    if (la > 0 && sa[la-1] == '/') la--;
    int lb = (int)strlen(sb);
    if (la + 1 + lb + 1 > out_size) {
        so[0] = '\0';
        return;
    }
    memcpy(so, sa, la);
    so[la] = '/';
    memcpy(so + la + 1, sb, lb);
    so[la + 1 + lb] = '\0';
}

void m2sys_home_dir(void *out, int32_t out_size) {
    const char *h = getenv("HOME");
    if (!h) h = "/tmp";
    strncpy((char *)out, h, out_size - 1);
    ((char *)out)[out_size - 1] = '\0';
}

void m2sys_getcwd(void *out, int32_t out_size) {
    if (!getcwd((char *)out, out_size)) {
        ((char *)out)[0] = '\0';
    }
}

int32_t m2sys_chdir(void *path) {
    return chdir((const char *)path) == 0 ? 0 : -1;
}

void m2sys_getenv(void *name, void *out, int32_t out_size) {
    const char *v = getenv((const char *)name);
    if (v) {
        strncpy((char *)out, v, out_size - 1);
        ((char *)out)[out_size - 1] = '\0';
    } else {
        ((char *)out)[0] = '\0';
    }
}

int32_t m2sys_strlen(void *s) {
    return (int32_t)strlen((const char *)s);
}

int32_t m2sys_str_eq(const void *a, const void *b) {
    return strcmp((const char *)a, (const char *)b) == 0 ? 1 : 0;
}

int32_t m2sys_str_starts_with(const void *s, const void *prefix) {
    const char *sp = (const char *)s;
    const char *pp = (const char *)prefix;
    while (*pp) {
        if (*sp != *pp) return 0;
        sp++; pp++;
    }
    return 1;
}

int32_t m2sys_str_append(void *dst, int32_t dst_size, const void *src) {
    char *d = (char *)dst;
    const char *s = (const char *)src;
    int32_t dlen = (int32_t)strlen(d);
    while (*s && dlen + 1 < dst_size) {
        d[dlen++] = *s++;
    }
    d[dlen] = '\0';
    return dlen;
}

int32_t m2sys_str_contains_ci(const void *haystack, const void *needle) {
    const char *h = (const char *)haystack;
    const char *n = (const char *)needle;
    if (!*n) return 1;
    for (; *h; h++) {
        const char *hp = h, *np = n;
        while (*np) {
            if (!*hp) goto next;
            char hc = (*hp >= 'A' && *hp <= 'Z') ? *hp + 32 : *hp;
            char nc = (*np >= 'A' && *np <= 'Z') ? *np + 32 : *np;
            if (hc != nc) goto next;
            hp++; np++;
        }
        return 1;
        next:;
    }
    return 0;
}

/* ── File operations ───────────────────────────────────────────── */

#include <fcntl.h>
#include <dirent.h>
#include <sys/file.h>
#include <libgen.h>

int32_t m2sys_sha256_file(void *path, void *hex_out) {
    FILE *fp = fopen((const char *)path, "rb");
    if (!fp) return -1;
    /* Read entire file, hash it */
    fseek(fp, 0, SEEK_END);
    long sz = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    uint8_t *buf = (uint8_t *)malloc(sz > 0 ? sz : 1);
    if (!buf) { fclose(fp); return -1; }
    size_t rd = fread(buf, 1, sz, fp);
    fclose(fp);
    m2sys_sha256_str(buf, (int32_t)rd, hex_out);
    free(buf);
    return 0;
}

int32_t m2sys_file_size(void *path) {
    struct stat st;
    if (stat((const char *)path, &st) != 0) return -1;
    return (int32_t)st.st_size;
}

int32_t m2sys_copy_file(void *src, void *dst) {
    FILE *in = fopen((const char *)src, "rb");
    if (!in) return -1;
    FILE *out = fopen((const char *)dst, "wb");
    if (!out) { fclose(in); return -1; }
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof(buf), in)) > 0) {
        if (fwrite(buf, 1, n, out) != n) {
            fclose(in); fclose(out); return -1;
        }
    }
    fclose(in);
    fclose(out);
    return 0;
}

int32_t m2sys_rename(void *old_path, void *new_path) {
    return rename((const char *)old_path, (const char *)new_path) == 0 ? 0 : -1;
}

/* ── Directory listing ─────────────────────────────────────────── */

int32_t m2sys_list_dir(void *dir, void *buf, int32_t bufSize) {
    DIR *d = opendir((const char *)dir);
    if (!d) return -1;
    char *out = (char *)buf;
    int32_t pos = 0;
    struct dirent *ent;
    while ((ent = readdir(d)) != NULL) {
        if (ent->d_name[0] == '.' &&
            (ent->d_name[1] == '\0' ||
             (ent->d_name[1] == '.' && ent->d_name[2] == '\0')))
            continue;
        int32_t nlen = (int32_t)strlen(ent->d_name);
        if (pos + nlen + 1 >= bufSize) break;
        memcpy(out + pos, ent->d_name, nlen);
        pos += nlen;
        out[pos++] = '\n';
    }
    if (pos > 0) pos--; /* remove trailing newline */
    out[pos] = '\0';
    closedir(d);
    return pos;
}

/* ── Path utilities ────────────────────────────────────────────── */

void m2sys_basename(void *path, void *out, int32_t outSize) {
    char tmp[1024];
    strncpy(tmp, (const char *)path, sizeof(tmp) - 1);
    tmp[sizeof(tmp) - 1] = '\0';
    const char *b = basename(tmp);
    strncpy((char *)out, b, outSize - 1);
    ((char *)out)[outSize - 1] = '\0';
}

void m2sys_dirname(void *path, void *out, int32_t outSize) {
    char tmp[1024];
    strncpy(tmp, (const char *)path, sizeof(tmp) - 1);
    tmp[sizeof(tmp) - 1] = '\0';
    const char *d = dirname(tmp);
    strncpy((char *)out, d, outSize - 1);
    ((char *)out)[outSize - 1] = '\0';
}

/* ── Process execution (captures stdout) ───────────────────────── */

int32_t m2sys_exec_output(void *cmdline, void *outBuf, int32_t outSize) {
    FILE *fp = popen((const char *)cmdline, "r");
    if (!fp) return -1;
    char *out = (char *)outBuf;
    int32_t pos = 0;
    int c;
    while ((c = fgetc(fp)) != EOF && pos < outSize - 1) {
        out[pos++] = (char)c;
    }
    out[pos] = '\0';
    int status = pclose(fp);
#ifdef _WIN32
    return (int32_t)status;
#else
    return WIFEXITED(status) ? (int32_t)WEXITSTATUS(status) : -1;
#endif
}

/* ── Tar (shells out to tar) ───────────────────────────────────── */

int32_t m2sys_tar_create(void *archivePath, void *baseDir) {
    char cmd[2048];
    snprintf(cmd, sizeof(cmd), "tar cf \"%s\" -C \"%s\" .",
             (const char *)archivePath, (const char *)baseDir);
    return m2sys_exec(cmd);
}

int32_t m2sys_tar_create_ex(void *archivePath, void *baseDir, void *excludePattern) {
    char cmd[2048];
    const char *excl = (const char *)excludePattern;
    if (excl && excl[0]) {
        snprintf(cmd, sizeof(cmd), "tar cf \"%s\" --exclude \"%s\" -C \"%s\" .",
                 (const char *)archivePath, excl, (const char *)baseDir);
    } else {
        snprintf(cmd, sizeof(cmd), "tar cf \"%s\" -C \"%s\" .",
                 (const char *)archivePath, (const char *)baseDir);
    }
    return m2sys_exec(cmd);
}

int32_t m2sys_tar_extract(void *archivePath, void *destDir) {
    char cmd[2048];
    int32_t rc = m2sys_mkdir_p(destDir);
    if (rc != 0) return rc;
    snprintf(cmd, sizeof(cmd), "tar xf \"%s\" -C \"%s\"",
             (const char *)archivePath, (const char *)destDir);
    return m2sys_exec(cmd);
}

/* ── File locking (POSIX flock) ────────────────────────────────── */

int32_t m2sys_flock(int32_t handle, int32_t exclusive) {
    if (handle < 0 || handle >= MAX_HANDLES || !handle_table[handle]) return -1;
    int fd = fileno(handle_table[handle]);
    return flock(fd, exclusive ? LOCK_EX : LOCK_SH) == 0 ? 0 : -1;
}

int32_t m2sys_funlock(int32_t handle) {
    if (handle < 0 || handle >= MAX_HANDLES || !handle_table[handle]) return -1;
    int fd = fileno(handle_table[handle]);
    return flock(fd, LOCK_UN) == 0 ? 0 : -1;
}

/* ── Remove directory recursively ──────────────────────────────── */

int32_t m2sys_rmdir_r(void *path) {
    char cmd[1100];
    snprintf(cmd, sizeof(cmd), "rm -rf \"%s\"", (const char *)path);
    return m2sys_exec(cmd);
}

/* ── Time ────────────────────────────────────────────────────────── */

#include <time.h>

int64_t m2sys_unix_time(void) {
    return (int64_t)time(NULL);
}

/* ── File metadata ───────────────────────────────────────────────── */

int64_t m2sys_file_mtime(void *path) {
    struct stat st;
    if (stat((const char *)path, &st) != 0) return -1;
    return (int64_t)st.st_mtime;
}

int32_t m2sys_is_symlink(void *path) {
    struct stat st;
    if (lstat((const char *)path, &st) != 0) return 0;
    return S_ISLNK(st.st_mode) ? 1 : 0;
}
