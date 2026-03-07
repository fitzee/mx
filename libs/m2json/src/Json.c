/* Modula-2 Runtime Support */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdint.h>
#include <ctype.h>
#include <limits.h>
#include <float.h>
#include <setjmp.h>

/* Command-line argument storage */
static int m2_argc = 0;
static char **m2_argv = NULL;

/* ISO Modula-2 exception handling support */
static jmp_buf m2_exception_buf;
static int m2_exception_code = 0;
static int m2_exception_active = 0;

/* Modula-2+ enhanced exception handling (setjmp/longjmp frame stack) */
typedef struct m2_ExcFrame {
    jmp_buf buf;
    struct m2_ExcFrame *prev;
    int exception_id;
    const char *exception_name;
    void *exception_arg;
} m2_ExcFrame;

static __thread m2_ExcFrame *m2_exc_stack = NULL;

/* Stack-based exception frame macros — no heap allocation.
   Usage:  m2_ExcFrame _ef;
           M2_TRY(_ef) { body; M2_ENDTRY(_ef); }
           M2_CATCH { M2_ENDTRY(_ef); handlers; }           */
#define M2_TRY(frame) \
    (frame).prev = m2_exc_stack; \
    (frame).exception_id = 0; \
    (frame).exception_name = NULL; \
    (frame).exception_arg = NULL; \
    m2_exc_stack = &(frame); \
    if (setjmp((frame).buf) == 0)

#define M2_CATCH else

#define M2_ENDTRY(frame) \
    m2_exc_stack = (frame).prev

static inline void m2_raise(int id, const char *name, void *arg) {
    if (m2_exc_stack) {
        m2_exc_stack->exception_id = id;
        m2_exc_stack->exception_name = name;
        m2_exc_stack->exception_arg = arg;
        longjmp(m2_exc_stack->buf, id ? id : 1);
    }
    /* Fallback to ISO exception mechanism */
    if (m2_exception_active) {
        m2_exception_code = id ? id : 1;
        longjmp(m2_exception_buf, m2_exception_code);
    }
    /* No handler — terminate */
    fprintf(stderr, "Unhandled exception: %s (id=%d)\n", name ? name : "unknown", id);
    exit(1);
}

/* Runtime type information (for TYPECASE / OBJECT) */
typedef struct M2_TypeDesc {
    uint32_t   type_id;
    const char *type_name;
    struct M2_TypeDesc *parent;
    uint32_t   depth;
} M2_TypeDesc;

/* Allocation header prepended before payload for typed REF/OBJECT allocations */
typedef struct M2_RefHeader {
#ifdef M2_RTTI_DEBUG
    uint32_t magic;   /* 0x4D325246 ("M2RF") */
    uint32_t flags;   /* 0 = live, 0xDEADDEAD = freed */
#endif
    M2_TypeDesc *td;
} M2_RefHeader;

#define M2_REFHEADER_MAGIC 0x4D325246u

/* Modula-2+ Thread support (pthreads) */
#ifdef M2_USE_THREADS
#include <pthread.h>
typedef struct m2_Thread {
    pthread_t handle;
    int alerted;
    pthread_mutex_t alert_mu;
} m2_Thread;

static __thread m2_Thread *m2_current_thread = NULL;

/* Thread.Fork — create a new thread from a parameterless procedure */
typedef void (*m2_ThreadProc)(void);
struct m2_thread_start_arg { m2_ThreadProc proc; m2_Thread *self; };

static void *m2_thread_start(void *arg) {
    struct m2_thread_start_arg *a = (struct m2_thread_start_arg *)arg;
    m2_current_thread = a->self;
    a->proc();
    free(a);
    return NULL;
}

static m2_Thread *m2_Thread_Fork(m2_ThreadProc proc) {
    m2_Thread *t = (m2_Thread *)malloc(sizeof(m2_Thread));
    t->alerted = 0;
    pthread_mutex_init(&t->alert_mu, NULL);
    struct m2_thread_start_arg *arg = (struct m2_thread_start_arg *)malloc(sizeof(struct m2_thread_start_arg));
    arg->proc = proc;
    arg->self = t;
    pthread_create(&t->handle, NULL, m2_thread_start, arg);
    return t;
}

static void m2_Thread_Join(m2_Thread *t) {
    pthread_join(t->handle, NULL);
}

static m2_Thread *m2_Thread_Self(void) {
    return m2_current_thread;
}

static void m2_Thread_Alert(m2_Thread *t) {
    pthread_mutex_lock(&t->alert_mu);
    t->alerted = 1;
    pthread_mutex_unlock(&t->alert_mu);
}

static int m2_Thread_TestAlert(void) {
    if (!m2_current_thread) return 0;
    pthread_mutex_lock(&m2_current_thread->alert_mu);
    int a = m2_current_thread->alerted;
    m2_current_thread->alerted = 0;
    pthread_mutex_unlock(&m2_current_thread->alert_mu);
    return a;
}

/* Mutex module */
typedef pthread_mutex_t *m2_Mutex_T;

static m2_Mutex_T m2_Mutex_New(void) {
    pthread_mutex_t *m = (pthread_mutex_t *)malloc(sizeof(pthread_mutex_t));
    pthread_mutex_init(m, NULL);
    return m;
}

static void m2_Mutex_Lock(m2_Mutex_T m) { pthread_mutex_lock(m); }
static void m2_Mutex_Unlock(m2_Mutex_T m) { pthread_mutex_unlock(m); }
static void m2_Mutex_Free(m2_Mutex_T m) { pthread_mutex_destroy(m); free(m); }

/* Condition module */
typedef pthread_cond_t *m2_Condition_T;

static m2_Condition_T m2_Condition_New(void) {
    pthread_cond_t *c = (pthread_cond_t *)malloc(sizeof(pthread_cond_t));
    pthread_cond_init(c, NULL);
    return c;
}

static void m2_Condition_Wait(m2_Condition_T c, m2_Mutex_T m) { pthread_cond_wait(c, m); }
static void m2_Condition_Signal(m2_Condition_T c) { pthread_cond_signal(c); }
static void m2_Condition_Broadcast(m2_Condition_T c) { pthread_cond_broadcast(c); }
static void m2_Condition_Free(m2_Condition_T c) { pthread_cond_destroy(c); free(c); }
#endif /* M2_USE_THREADS */

/* Modula-2+ Garbage Collection support (Boehm GC) */
#if defined(M2_USE_GC) && __has_include(<gc/gc.h>)
#include <gc/gc.h>
#else
/* Fallback: use malloc when GC is not available */
#ifdef M2_USE_GC
#undef M2_USE_GC
#endif
#define GC_MALLOC(sz) malloc(sz)
#define GC_REALLOC(p, sz) realloc(p, sz)
#define GC_FREE(p) free(p)
static inline void GC_INIT(void) {}
#endif

/* Allocate a GC-traced REF/OBJECT with M2_RefHeader prepended before payload */
static inline void *M2_ref_alloc(size_t payload_size, M2_TypeDesc *td) {
    M2_RefHeader *hdr = (M2_RefHeader *)GC_MALLOC(sizeof(M2_RefHeader) + payload_size);
    if (!hdr) { fprintf(stderr, "M2_ref_alloc: out of memory\n"); exit(1); }
#ifdef M2_RTTI_DEBUG
    hdr->magic = M2_REFHEADER_MAGIC;
    hdr->flags = 0;
#endif
    hdr->td = td;
    return (void *)(hdr + 1); /* return pointer to payload (past header) */
}

/* Recover the type descriptor from a typed REF/REFANY payload pointer.
   Returns NULL if ref is NULL or (in debug mode) if the header is invalid. */
static inline M2_TypeDesc *M2_TYPEOF(void *ref) {
    if (!ref) return NULL;
    M2_RefHeader *hdr = ((M2_RefHeader *)ref) - 1;
#ifdef M2_RTTI_DEBUG
    if (hdr->magic != M2_REFHEADER_MAGIC) return NULL;
    if (hdr->flags == 0xDEADDEADu) {
        fprintf(stderr, "M2_TYPEOF: use-after-free detected\n");
        return NULL;
    }
#endif
    return hdr->td;
}

/* Check if a payload's type is (or inherits from) a target type descriptor.
   Returns 1 if match, 0 otherwise. Safe with NULL payloads. */
static inline int M2_ISA(void *payload, M2_TypeDesc *target) {
    M2_TypeDesc *td = M2_TYPEOF(payload);
    if (!td || !target) return 0;
    if (td->depth < target->depth) return 0; /* early-out: can't be a subtype */
    while (td) {
        if (td == target) return 1;
        td = td->parent;
    }
    return 0;
}

/* Narrow: returns payload if it matches target type, otherwise raises an exception */
static inline void *M2_NARROW(void *payload, M2_TypeDesc *target) {
    if (M2_ISA(payload, target)) return payload;
    m2_raise(99, "NarrowFault", NULL);
    return NULL; /* unreachable */
}

/* Free a typed REF object — poisons header in debug mode */
static inline void M2_ref_free(void *payload) {
    if (!payload) return;
    M2_RefHeader *hdr = ((M2_RefHeader *)payload) - 1;
#ifdef M2_RTTI_DEBUG
    hdr->flags = 0xDEADDEADu;
#endif
    GC_FREE(hdr);
}

/* PIM4 DIV: floored division (truncates toward negative infinity) */
static inline int32_t m2_div(int32_t a, int32_t b) {
    int32_t q = a / b;
    int32_t r = a % b;
    if ((r != 0) && ((r ^ b) < 0)) q--;
    return q;
}

/* PIM4 MOD: result is always non-negative (when b > 0) */
static inline int32_t m2_mod(int32_t a, int32_t b) {
    int32_t r = a % b;
    if (r < 0) r += (b > 0 ? b : -b);
    return r;
}

/* ISO Modula-2 COMPLEX types */
typedef struct { float re, im; } m2_COMPLEX;
typedef struct { double re, im; } m2_LONGCOMPLEX;

static inline m2_COMPLEX m2_complex_add(m2_COMPLEX a, m2_COMPLEX b) {
    return (m2_COMPLEX){ a.re + b.re, a.im + b.im };
}
static inline m2_COMPLEX m2_complex_sub(m2_COMPLEX a, m2_COMPLEX b) {
    return (m2_COMPLEX){ a.re - b.re, a.im - b.im };
}
static inline m2_COMPLEX m2_complex_mul(m2_COMPLEX a, m2_COMPLEX b) {
    return (m2_COMPLEX){ a.re*b.re - a.im*b.im, a.re*b.im + a.im*b.re };
}
static inline m2_COMPLEX m2_complex_div(m2_COMPLEX a, m2_COMPLEX b) {
    float d = b.re*b.re + b.im*b.im;
    return (m2_COMPLEX){ (a.re*b.re + a.im*b.im)/d, (a.im*b.re - a.re*b.im)/d };
}
static inline int m2_complex_eq(m2_COMPLEX a, m2_COMPLEX b) {
    return a.re == b.re && a.im == b.im;
}
static inline m2_COMPLEX m2_complex_neg(m2_COMPLEX a) {
    return (m2_COMPLEX){ -a.re, -a.im };
}
static inline float m2_complex_abs(m2_COMPLEX a) {
    return sqrtf(a.re*a.re + a.im*a.im);
}
static inline m2_LONGCOMPLEX m2_lcomplex_add(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return (m2_LONGCOMPLEX){ a.re + b.re, a.im + b.im };
}
static inline m2_LONGCOMPLEX m2_lcomplex_sub(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return (m2_LONGCOMPLEX){ a.re - b.re, a.im - b.im };
}
static inline m2_LONGCOMPLEX m2_lcomplex_mul(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return (m2_LONGCOMPLEX){ a.re*b.re - a.im*b.im, a.re*b.im + a.im*b.re };
}
static inline m2_LONGCOMPLEX m2_lcomplex_div(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    double d = b.re*b.re + b.im*b.im;
    return (m2_LONGCOMPLEX){ (a.re*b.re + a.im*b.im)/d, (a.im*b.re - a.re*b.im)/d };
}
static inline int m2_lcomplex_eq(m2_LONGCOMPLEX a, m2_LONGCOMPLEX b) {
    return a.re == b.re && a.im == b.im;
}
static inline m2_LONGCOMPLEX m2_lcomplex_neg(m2_LONGCOMPLEX a) {
    return (m2_LONGCOMPLEX){ -a.re, -a.im };
}
static inline double m2_lcomplex_abs(m2_LONGCOMPLEX a) {
    return sqrt(a.re*a.re + a.im*a.im);
}

/* Built-in MAX/MIN - type-generic via macros */
#define m2_max_INTEGER INT32_MAX
#define m2_max_CARDINAL UINT32_MAX
#define m2_max_CHAR 255
#define m2_max_BOOLEAN 1
#define m2_max_REAL FLT_MAX
#define m2_max_LONGREAL DBL_MAX
#define m2_max_BITSET 31
#define m2_max_LONGINT INT64_MAX
#define m2_max_LONGCARD UINT64_MAX
#define m2_min_INTEGER INT32_MIN
#define m2_min_CARDINAL 0
#define m2_min_CHAR 0
#define m2_min_BOOLEAN 0
#define m2_min_REAL FLT_MIN
#define m2_min_LONGREAL DBL_MIN
#define m2_min_BITSET 0
#define m2_min_LONGINT INT64_MIN
#define m2_min_LONGCARD 0
#define m2_max(T) m2_max_##T
#define m2_min(T) m2_min_##T

/* ISO SYSTEM.SHIFT — positive n shifts left, negative shifts right, vacated bits = 0 */
static inline uint32_t m2_shift(uint32_t val, int32_t n) {
    if (n == 0) return val;
    if (n > 0) return (n >= 32) ? 0u : (val << n);
    n = -n;
    return (n >= 32) ? 0u : (val >> n);
}
/* ISO SYSTEM.ROTATE — positive n rotates left, negative rotates right */
static inline uint32_t m2_rotate(uint32_t val, int32_t n) {
    n = n % 32;
    if (n < 0) n += 32;
    if (n == 0) return val;
    return (val << n) | (val >> (32 - n));
}

/* InOut module */
static int m2_InOut_Done = 1;
static void m2_WriteString(const char *s) { printf("%s", s); }
static void m2_WriteLn(void) { printf("\n"); }
static void m2_WriteInt(int32_t n, int32_t w) { printf("%*d", (int)w, (int)n); }
static void m2_WriteCard(uint32_t n, int32_t w) { printf("%*u", (int)w, (unsigned)n); }
static void m2_WriteHex(uint32_t n, int32_t w) { printf("%*X", (int)w, (unsigned)n); }
static void m2_WriteOct(uint32_t n, int32_t w) { printf("%*o", (int)w, (unsigned)n); }
static void m2_Write(char ch) { putchar(ch); }
static void m2_Read(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; m2_InOut_Done = (c != EOF); }
static void m2_ReadString(char *s) { m2_InOut_Done = (scanf("%s", s) == 1); }
static void m2_ReadInt(int32_t *n) { m2_InOut_Done = (scanf("%d", n) == 1); }
static void m2_ReadCard(uint32_t *n) { m2_InOut_Done = (scanf("%u", n) == 1); }

static FILE *m2_InFile = NULL;
static FILE *m2_OutFile = NULL;
static void m2_OpenInput(const char *ext) {
    char name[256];
    printf("Input file: "); scanf("%255s", name);
    if (ext && ext[0]) { strcat(name, "."); strcat(name, ext); }
    m2_InFile = fopen(name, "r");
    m2_InOut_Done = (m2_InFile != NULL);
}
static void m2_OpenOutput(const char *ext) {
    char name[256];
    printf("Output file: "); scanf("%255s", name);
    if (ext && ext[0]) { strcat(name, "."); strcat(name, ext); }
    m2_OutFile = fopen(name, "w");
    m2_InOut_Done = (m2_OutFile != NULL);
}
static void m2_CloseInput(void) { if (m2_InFile) { fclose(m2_InFile); m2_InFile = NULL; } }
static void m2_CloseOutput(void) { if (m2_OutFile) { fclose(m2_OutFile); m2_OutFile = NULL; } }

/* RealInOut module */
static int m2_RealInOut_Done = 1;
static void m2_ReadReal(float *r) { m2_RealInOut_Done = (scanf("%f", r) == 1); }
static void m2_WriteReal(float r, int32_t w) { printf("%*g", (int)w, (double)r); }
static void m2_WriteFixPt(float r, int32_t w, int32_t d) { printf("%*.*f", (int)w, (int)d, (double)r); }
static void m2_WriteRealOct(float r) { printf("%.8A", (double)r); }

/* Storage module */
static void m2_ALLOCATE(void **p, uint32_t size) { *p = malloc(size); }
static void m2_DEALLOCATE(void **p, uint32_t size) { free(*p); *p = NULL; (void)size; }

/* Strings module — bounded, always NUL-terminates, truncates on overflow */
static void m2_Strings_Assign(const char *src, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(src);
    if (slen >= cap) slen = cap - 1;
    memcpy(dst, src, slen);
    dst[slen] = '\0';
}
static void m2_Strings_Insert(const char *sub, char *dst, uint32_t dst_high, uint32_t pos) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(sub), dlen = strlen(dst);
    if (pos > dlen) pos = (uint32_t)dlen;
    size_t new_len = dlen + slen;
    if (new_len >= cap) new_len = cap - 1;
    /* how much of the tail after pos can we keep? */
    size_t tail_dst = pos + slen;
    size_t tail_keep = (tail_dst < new_len) ? new_len - tail_dst : 0;
    if (tail_keep > 0)
        memmove(dst + tail_dst, dst + pos, tail_keep);
    /* how much of sub fits? */
    size_t sub_copy = slen;
    if (pos + sub_copy > new_len) sub_copy = new_len - pos;
    if (sub_copy > 0)
        memcpy(dst + pos, sub, sub_copy);
    dst[new_len] = '\0';
}
static void m2_Strings_Delete(char *s, uint32_t s_high, uint32_t pos, uint32_t len) {
    size_t slen = strlen(s);
    (void)s_high; /* delete only shrinks — can never overflow */
    if (pos >= slen) return;
    if (pos + len > slen) len = (uint32_t)(slen - pos);
    memmove(s + pos, s + pos + len, slen - pos - len + 1);
}
static uint32_t m2_Strings_Pos(const char *sub, const char *s) {
    const char *p = strstr(s, sub);
    return p ? (uint32_t)(p - s) : UINT32_MAX;
}
static uint32_t m2_Strings_Length(const char *s) { return (uint32_t)strlen(s); }
static void m2_Strings_Copy(const char *src, uint32_t pos, uint32_t len, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t slen = strlen(src);
    if (pos >= slen) { dst[0] = '\0'; return; }
    if (pos + len > slen) len = (uint32_t)(slen - pos);
    if (len >= cap) len = (uint32_t)(cap - 1);
    memcpy(dst, src + pos, len);
    dst[len] = '\0';
}
static void m2_Strings_Concat(const char *s1, const char *s2, char *dst, uint32_t dst_high) {
    size_t cap = (size_t)dst_high + 1;
    size_t len1 = strlen(s1), len2 = strlen(s2);
    if (len1 >= cap) len1 = cap - 1;
    memcpy(dst, s1, len1);
    size_t rem = cap - 1 - len1;
    if (len2 > rem) len2 = rem;
    memcpy(dst + len1, s2, len2);
    dst[len1 + len2] = '\0';
}
static int32_t m2_Strings_CompareStr(const char *s1, const char *s2) { return (int32_t)strcmp(s1, s2); }

/* Terminal module */
static int m2_Terminal_Done = 1;
static void m2_Terminal_Read(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; m2_Terminal_Done = (c != EOF); }
static void m2_Terminal_Write(char ch) { putchar(ch); }
static void m2_Terminal_WriteString(const char *s) { printf("%s", s); }
static void m2_Terminal_WriteLn(void) { printf("\n"); }

/* FileSystem module */
typedef FILE *m2_File;
static int m2_FileSystem_Done = 1;
static void m2_Lookup(m2_File *f, const char *name, int newFile) {
    *f = fopen(name, newFile ? "w+" : "r+");
    if (!*f && !newFile) *f = fopen(name, "r");
    m2_FileSystem_Done = (*f != NULL);
}
static void m2_Close(m2_File *f) { if (*f) { fclose(*f); *f = NULL; } }
static void m2_ReadChar(m2_File *f, char *ch) {
    int c = fgetc(*f);
    *ch = (c == EOF) ? '\0' : (char)c;
    m2_FileSystem_Done = (c != EOF);
}
static void m2_WriteChar(m2_File *f, char ch) {
    fputc(ch, *f);
}

/* SYSTEM module */
#define m2_ADR(x) ((void *)&(x))
#define m2_TSIZE(T) ((uint32_t)sizeof(T))

/* ISO STextIO module */
static void m2_STextIO_WriteChar(char ch) { putchar(ch); }
static void m2_STextIO_ReadChar(char *ch) { int c = getchar(); *ch = (c == EOF) ? '\0' : (char)c; }
static void m2_STextIO_WriteString(const char *s) { printf("%s", s); }
static void m2_STextIO_ReadString(char *s, uint32_t s_high) {
    if (fgets(s, (int)(s_high + 1), stdin) == NULL) s[0] = '\0';
    /* strip trailing newline */
    size_t len = strlen(s);
    if (len > 0 && s[len-1] == '\n') s[len-1] = '\0';
}
static void m2_STextIO_WriteLn(void) { putchar('\n'); }
static void m2_STextIO_SkipLine(void) { int c; while ((c = getchar()) != '\n' && c != EOF); }
static void m2_STextIO_ReadToken(char *s, uint32_t s_high) { m2_STextIO_ReadString(s, s_high); }

/* ISO SWholeIO module */
static void m2_SWholeIO_WriteInt(int32_t n, uint32_t w) { printf("%*d", (int)w, (int)n); }
static void m2_SWholeIO_ReadInt(int32_t *n) { scanf("%d", (int *)n); }
static void m2_SWholeIO_WriteCard(uint32_t n, uint32_t w) { printf("%*u", (int)w, (unsigned)n); }
static void m2_SWholeIO_ReadCard(uint32_t *n) { scanf("%u", (unsigned *)n); }

/* ISO SRealIO module */
static void m2_SRealIO_WriteFloat(float r, uint32_t sigFigs, uint32_t w) {
    printf("%*.*e", (int)w, (int)sigFigs, (double)r);
}
static void m2_SRealIO_WriteFixed(float r, int32_t place, uint32_t w) {
    printf("%*.*f", (int)w, (int)place, (double)r);
}
static void m2_SRealIO_WriteReal(float r, uint32_t w) { printf("%*g", (int)w, (double)r); }
static void m2_SRealIO_ReadReal(float *r) { double d; scanf("%lf", &d); *r = (float)d; }

/* ISO SLongIO module */
static void m2_SLongIO_WriteFloat(double r, uint32_t sigFigs, uint32_t w) {
    printf("%*.*e", (int)w, (int)sigFigs, r);
}
static void m2_SLongIO_WriteFixed(double r, int32_t place, uint32_t w) {
    printf("%*.*f", (int)w, (int)place, r);
}
static void m2_SLongIO_WriteLongReal(double r, uint32_t w) { printf("%*g", (int)w, r); }
static void m2_SLongIO_ReadLongReal(double *r) { scanf("%lf", r); }

/* Args module */
static uint32_t m2_Args_ArgCount(void) { return (uint32_t)m2_argc; }
static void m2_Args_GetArg(uint32_t n, char *buf, uint32_t buf_high) {
    (void)buf_high;
    if ((int)n < m2_argc) {
        strncpy(buf, m2_argv[n], buf_high + 1);
        buf[buf_high] = '\0';
    } else {
        buf[0] = '\0';
    }
}

/* BinaryIO module - file handle table using FILE* pointers */
#define M2_MAX_FILES 32
static FILE *m2_bio_files[M2_MAX_FILES];
static int m2_bio_init = 0;
static int m2_BinaryIO_Done = 1;

static void m2_bio_ensure_init(void) {
    if (!m2_bio_init) {
        for (int i = 0; i < M2_MAX_FILES; i++) m2_bio_files[i] = NULL;
        m2_bio_init = 1;
    }
}

static int m2_bio_alloc(void) {
    m2_bio_ensure_init();
    for (int i = 0; i < M2_MAX_FILES; i++) {
        if (m2_bio_files[i] == NULL) return i;
    }
    return -1;
}

static void m2_BinaryIO_OpenRead(const char *name, uint32_t *fh) {
    int slot = m2_bio_alloc();
    if (slot < 0) { m2_BinaryIO_Done = 0; *fh = 0; return; }
    m2_bio_files[slot] = fopen(name, "rb");
    if (m2_bio_files[slot]) { *fh = (uint32_t)(slot + 1); m2_BinaryIO_Done = 1; }
    else { *fh = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_OpenWrite(const char *name, uint32_t *fh) {
    int slot = m2_bio_alloc();
    if (slot < 0) { m2_BinaryIO_Done = 0; *fh = 0; return; }
    m2_bio_files[slot] = fopen(name, "wb");
    if (m2_bio_files[slot]) { *fh = (uint32_t)(slot + 1); m2_BinaryIO_Done = 1; }
    else { *fh = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_Close(uint32_t fh) {
    m2_bio_ensure_init();
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fclose(m2_bio_files[fh-1]);
        m2_bio_files[fh-1] = NULL;
    }
}

static void m2_BinaryIO_ReadByte(uint32_t fh, uint32_t *b) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        int c = fgetc(m2_bio_files[fh-1]);
        if (c == EOF) { *b = 0; m2_BinaryIO_Done = 0; }
        else { *b = (uint32_t)(unsigned char)c; m2_BinaryIO_Done = 1; }
    } else { *b = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_WriteByte(uint32_t fh, uint32_t b) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fputc((unsigned char)(b & 0xFF), m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_ReadBytes(uint32_t fh, char *buf, uint32_t n, uint32_t *actual) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        *actual = (uint32_t)fread(buf, 1, n, m2_bio_files[fh-1]);
        m2_BinaryIO_Done = (*actual > 0) ? 1 : 0;
    } else { *actual = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_WriteBytes(uint32_t fh, const char *buf, uint32_t n) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fwrite(buf, 1, n, m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_FileSize(uint32_t fh, uint32_t *size) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        long cur = ftell(m2_bio_files[fh-1]);
        fseek(m2_bio_files[fh-1], 0, SEEK_END);
        *size = (uint32_t)ftell(m2_bio_files[fh-1]);
        fseek(m2_bio_files[fh-1], cur, SEEK_SET);
        m2_BinaryIO_Done = 1;
    } else { *size = 0; m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_Seek(uint32_t fh, uint32_t pos) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        fseek(m2_bio_files[fh-1], (long)pos, SEEK_SET);
        m2_BinaryIO_Done = 1;
    } else { m2_BinaryIO_Done = 0; }
}

static void m2_BinaryIO_Tell(uint32_t fh, uint32_t *pos) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        *pos = (uint32_t)ftell(m2_bio_files[fh-1]);
        m2_BinaryIO_Done = 1;
    } else { *pos = 0; m2_BinaryIO_Done = 0; }
}

static int m2_BinaryIO_IsEOF(uint32_t fh) {
    if (fh >= 1 && fh <= M2_MAX_FILES && m2_bio_files[fh-1]) {
        return feof(m2_bio_files[fh-1]) ? 1 : 0;
    }
    return 1;
}

/* Implementation Module Json */

typedef struct Token Token;
typedef struct Parser Parser;
typedef enum { TokenKind_JNull, TokenKind_JTrue, TokenKind_JFalse, TokenKind_JNumber, TokenKind_JString, TokenKind_JArrayStart, TokenKind_JArrayEnd, TokenKind_JObjectStart, TokenKind_JObjectEnd, TokenKind_JColon, TokenKind_JComma, TokenKind_JError, TokenKind_JEnd } TokenKind;

struct Token {
    TokenKind kind;
    uint32_t start;
    uint32_t len;
};

typedef char SrcArray[1048575 + 1];

typedef SrcArray *SrcPtr;

struct Parser {
    SrcPtr src;
    uint32_t srcLen;
    uint32_t pos;
    char err[127 + 1];
    int hasError;
};

char CharAt(Parser *p, uint32_t idx);
int IsWhitespace(char ch);
int IsDigit(char ch);
void SkipWS(Parser *p);
void SetError(Parser *p, char *msg, uint32_t msg_high);
void CopyStr(char *src, uint32_t src_high, char *dst, uint32_t dst_high);
int HexVal(char ch, uint32_t *val);
void Init(Parser *p, void * src, uint32_t srcLen);
int ScanString(Parser *p, Token *tok);
int ScanNumber(Parser *p, Token *tok);
int MatchKeyword(Parser *p, char *kw, uint32_t kw_high);
int Next(Parser *p, Token *tok);
int GetString(Parser *p, Token *tok, char *buf, uint32_t buf_high);
int GetInteger(Parser *p, Token *tok, int32_t *val);
int GetReal(Parser *p, Token *tok, float *val);
void Skip(Parser *p);
void GetError(Parser *p, char *buf, uint32_t buf_high);


#line 7 "/Users/mattfitz/dev/m2/libs/m2json/src/Json.mod"
char CharAt(Parser *p, uint32_t idx) {
#line 9
    if ((idx >= (*p).srcLen)) {
        return '\0';
    }
#line 10
    return (*(*p).src)[idx];
}

#line 13
int IsWhitespace(char ch) {
#line 15
    return ((((ch == ' ') || (ch == ((char)(9)))) || (ch == ((char)(10)))) || (ch == ((char)(13))));
}

#line 18
int IsDigit(char ch) {
#line 20
    return ((ch >= '0') && (ch <= '9'));
}

#line 23
void SkipWS(Parser *p) {
#line 25
    while ((((*p).pos < (*p).srcLen) && IsWhitespace(CharAt(p, (*p).pos)))) {
#line 26
        ((*p).pos++);
    }
}

#line 30
void SetError(Parser *p, char *msg, uint32_t msg_high) {
    uint32_t i, lim;
#line 33
    (*p).hasError = 1;
#line 34
    lim = msg_high;
#line 35
    if ((lim > (sizeof((*p).err) / sizeof((*p).err[0])) - 1)) {
        lim = (sizeof((*p).err) / sizeof((*p).err[0])) - 1;
    }
#line 36
    i = 0;
#line 37
    while (((i <= lim) && (msg[i] != '\0'))) {
#line 38
        (*p).err[i] = msg[i];
#line 39
        (i++);
    }
#line 41
    if ((i <= (sizeof((*p).err) / sizeof((*p).err[0])) - 1)) {
        (*p).err[i] = '\0';
    }
}

#line 44
void CopyStr(char *src, uint32_t src_high, char *dst, uint32_t dst_high) {
    uint32_t i, lim;
#line 47
    lim = src_high;
#line 48
    if ((lim > dst_high)) {
        lim = dst_high;
    }
#line 49
    i = 0;
#line 50
    while (((i <= lim) && (src[i] != '\0'))) {
#line 51
        dst[i] = src[i];
#line 52
        (i++);
    }
#line 54
    if ((i <= dst_high)) {
        dst[i] = '\0';
    }
}

#line 59
int HexVal(char ch, uint32_t *val) {
#line 61
    if (((ch >= '0') && (ch <= '9'))) {
#line 62
        (*val) = (((uint32_t)((unsigned char)(ch))) - ((uint32_t)((unsigned char)('0'))));
#line 63
        return 1;
    } else if (((ch >= 'a') && (ch <= 'f'))) {
#line 65
        (*val) = ((((uint32_t)((unsigned char)(ch))) - ((uint32_t)((unsigned char)('a')))) + 10);
#line 66
        return 1;
    } else if (((ch >= 'A') && (ch <= 'F'))) {
#line 68
        (*val) = ((((uint32_t)((unsigned char)(ch))) - ((uint32_t)((unsigned char)('A')))) + 10);
#line 69
        return 1;
    }
#line 71
    return 0;
}

#line 76
void Init(Parser *p, void * src, uint32_t srcLen) {
#line 78
    (*p).src = src;
#line 79
    (*p).srcLen = srcLen;
#line 80
    (*p).pos = 0;
#line 81
    (*p).hasError = 0;
#line 82
    (*p).err[0] = '\0';
}

#line 87
int ScanString(Parser *p, Token *tok) {
    char ch;
#line 91
    ((*p).pos++);
#line 92
    (*tok).start = (*p).pos;
#line 93
    while (((*p).pos < (*p).srcLen)) {
#line 94
        ch = CharAt(p, (*p).pos);
#line 95
        if ((ch == '"')) {
#line 96
            (*tok).len = ((*p).pos - (*tok).start);
#line 97
            ((*p).pos++);
#line 98
            (*tok).kind = TokenKind_JString;
#line 99
            return 1;
        } else if ((ch == ((char)(92)))) {
#line 101
            ((*p).pos++);
#line 102
            if (((*p).pos >= (*p).srcLen)) {
#line 103
                SetError(p, "unexpected end in string escape", (sizeof("unexpected end in string escape") / sizeof("unexpected end in string escape"[0])) - 1);
#line 104
                (*tok).kind = TokenKind_JError;
#line 105
                (*tok).len = 0;
#line 106
                return 0;
            }
#line 108
            ((*p).pos++);
        } else {
#line 110
            ((*p).pos++);
        }
    }
#line 113
    SetError(p, "unterminated string", (sizeof("unterminated string") / sizeof("unterminated string"[0])) - 1);
#line 114
    (*tok).kind = TokenKind_JError;
#line 115
    (*tok).len = 0;
#line 116
    return 0;
}

#line 119
int ScanNumber(Parser *p, Token *tok) {
    uint32_t start;
#line 122
    start = (*p).pos;
#line 123
    (*tok).start = start;
#line 126
    if ((((*p).pos < (*p).srcLen) && (CharAt(p, (*p).pos) == '-'))) {
#line 127
        ((*p).pos++);
    }
#line 131
    if ((((*p).pos >= (*p).srcLen) || (!IsDigit(CharAt(p, (*p).pos))))) {
#line 132
        SetError(p, "expected digit in number", (sizeof("expected digit in number") / sizeof("expected digit in number"[0])) - 1);
#line 133
        (*tok).kind = TokenKind_JError;
#line 134
        (*tok).len = ((*p).pos - start);
#line 135
        return 0;
    }
#line 137
    if ((CharAt(p, (*p).pos) == '0')) {
#line 138
        ((*p).pos++);
    } else {
#line 140
        while ((((*p).pos < (*p).srcLen) && IsDigit(CharAt(p, (*p).pos)))) {
#line 141
            ((*p).pos++);
        }
    }
#line 146
    if ((((*p).pos < (*p).srcLen) && (CharAt(p, (*p).pos) == '.'))) {
#line 147
        ((*p).pos++);
#line 148
        if ((((*p).pos >= (*p).srcLen) || (!IsDigit(CharAt(p, (*p).pos))))) {
#line 149
            SetError(p, "expected digit after decimal point", (sizeof("expected digit after decimal point") / sizeof("expected digit after decimal point"[0])) - 1);
#line 150
            (*tok).kind = TokenKind_JError;
#line 151
            (*tok).len = ((*p).pos - start);
#line 152
            return 0;
        }
#line 154
        while ((((*p).pos < (*p).srcLen) && IsDigit(CharAt(p, (*p).pos)))) {
#line 155
            ((*p).pos++);
        }
    }
#line 160
    if ((((*p).pos < (*p).srcLen) && ((CharAt(p, (*p).pos) == 'e') || (CharAt(p, (*p).pos) == 'E')))) {
#line 162
        ((*p).pos++);
#line 163
        if ((((*p).pos < (*p).srcLen) && ((CharAt(p, (*p).pos) == '+') || (CharAt(p, (*p).pos) == '-')))) {
#line 165
            ((*p).pos++);
        }
#line 167
        if ((((*p).pos >= (*p).srcLen) || (!IsDigit(CharAt(p, (*p).pos))))) {
#line 168
            SetError(p, "expected digit in exponent", (sizeof("expected digit in exponent") / sizeof("expected digit in exponent"[0])) - 1);
#line 169
            (*tok).kind = TokenKind_JError;
#line 170
            (*tok).len = ((*p).pos - start);
#line 171
            return 0;
        }
#line 173
        while ((((*p).pos < (*p).srcLen) && IsDigit(CharAt(p, (*p).pos)))) {
#line 174
            ((*p).pos++);
        }
    }
#line 178
    (*tok).kind = TokenKind_JNumber;
#line 179
    (*tok).len = ((*p).pos - start);
#line 180
    return 1;
}

#line 183
int MatchKeyword(Parser *p, char *kw, uint32_t kw_high) {
    uint32_t i, kwLen;
#line 186
    kwLen = 0;
#line 187
    while (((kwLen <= kw_high) && (kw[kwLen] != '\0'))) {
        (kwLen++);
    }
#line 189
    if ((((*p).pos + kwLen) > (*p).srcLen)) {
        return 0;
    }
#line 190
    i = 0;
#line 191
    while ((i < kwLen)) {
#line 192
        if ((CharAt(p, ((*p).pos + i)) != kw[i])) {
            return 0;
        }
#line 193
        (i++);
    }
#line 195
    (*p).pos = ((*p).pos + kwLen);
#line 196
    return 1;
}

#line 199
int Next(Parser *p, Token *tok) {
    char ch;
#line 202
    if ((*p).hasError) {
#line 203
        (*tok).kind = TokenKind_JError;
#line 204
        (*tok).start = (*p).pos;
#line 205
        (*tok).len = 0;
#line 206
        return 0;
    }
#line 209
    SkipWS(p);
#line 211
    if (((*p).pos >= (*p).srcLen)) {
#line 212
        (*tok).kind = TokenKind_JEnd;
#line 213
        (*tok).start = (*p).pos;
#line 214
        (*tok).len = 0;
#line 215
        return 0;
    }
#line 218
    ch = CharAt(p, (*p).pos);
#line 221
    if ((ch == '{')) {
#line 222
        (*tok).kind = TokenKind_JObjectStart;
        (*tok).start = (*p).pos;
        (*tok).len = 1;
#line 223
        ((*p).pos++);
        return 1;
    } else if ((ch == '}')) {
#line 225
        (*tok).kind = TokenKind_JObjectEnd;
        (*tok).start = (*p).pos;
        (*tok).len = 1;
#line 226
        ((*p).pos++);
        return 1;
    } else if ((ch == '[')) {
#line 228
        (*tok).kind = TokenKind_JArrayStart;
        (*tok).start = (*p).pos;
        (*tok).len = 1;
#line 229
        ((*p).pos++);
        return 1;
    } else if ((ch == ']')) {
#line 231
        (*tok).kind = TokenKind_JArrayEnd;
        (*tok).start = (*p).pos;
        (*tok).len = 1;
#line 232
        ((*p).pos++);
        return 1;
    } else if ((ch == ':')) {
#line 234
        (*tok).kind = TokenKind_JColon;
        (*tok).start = (*p).pos;
        (*tok).len = 1;
#line 235
        ((*p).pos++);
        return 1;
    } else if ((ch == ',')) {
#line 237
        (*tok).kind = TokenKind_JComma;
        (*tok).start = (*p).pos;
        (*tok).len = 1;
#line 238
        ((*p).pos++);
        return 1;
    } else if ((ch == '"')) {
#line 242
        return ScanString(p, tok);
    } else if ((IsDigit(ch) || (ch == '-'))) {
#line 246
        return ScanNumber(p, tok);
    } else if ((ch == 't')) {
#line 250
        (*tok).start = (*p).pos;
#line 251
        if (MatchKeyword(p, "true", (sizeof("true") / sizeof("true"[0])) - 1)) {
#line 252
            (*tok).kind = TokenKind_JTrue;
            (*tok).len = 4;
            return 1;
        } else {
#line 254
            SetError(p, "invalid token", (sizeof("invalid token") / sizeof("invalid token"[0])) - 1);
#line 255
            (*tok).kind = TokenKind_JError;
            (*tok).len = 0;
            return 0;
        }
    } else if ((ch == 'f')) {
#line 258
        (*tok).start = (*p).pos;
#line 259
        if (MatchKeyword(p, "false", (sizeof("false") / sizeof("false"[0])) - 1)) {
#line 260
            (*tok).kind = TokenKind_JFalse;
            (*tok).len = 5;
            return 1;
        } else {
#line 262
            SetError(p, "invalid token", (sizeof("invalid token") / sizeof("invalid token"[0])) - 1);
#line 263
            (*tok).kind = TokenKind_JError;
            (*tok).len = 0;
            return 0;
        }
    } else if ((ch == 'n')) {
#line 266
        (*tok).start = (*p).pos;
#line 267
        if (MatchKeyword(p, "null", (sizeof("null") / sizeof("null"[0])) - 1)) {
#line 268
            (*tok).kind = TokenKind_JNull;
            (*tok).len = 4;
            return 1;
        } else {
#line 270
            SetError(p, "invalid token", (sizeof("invalid token") / sizeof("invalid token"[0])) - 1);
#line 271
            (*tok).kind = TokenKind_JError;
            (*tok).len = 0;
            return 0;
        }
    } else {
#line 274
        (*tok).start = (*p).pos;
#line 275
        (*tok).len = 1;
#line 276
        (*tok).kind = TokenKind_JError;
#line 277
        SetError(p, "unexpected character", (sizeof("unexpected character") / sizeof("unexpected character"[0])) - 1);
#line 278
        return 0;
    }
}

#line 284
int GetString(Parser *p, Token *tok, char *buf, uint32_t buf_high) {
    uint32_t i, out, limit;
    char ch, esc;
    uint32_t h, hv, cp;
#line 291
    if (((*tok).kind != TokenKind_JString)) {
#line 292
        if ((0 <= buf_high)) {
            buf[0] = '\0';
        }
#line 293
        return 0;
    }
#line 296
    out = 0;
#line 297
    limit = buf_high;
#line 298
    i = (*tok).start;
#line 300
    while ((i < ((*tok).start + (*tok).len))) {
#line 301
        ch = (*(*p).src)[i];
#line 302
        if ((ch == ((char)(92)))) {
#line 303
            (i++);
#line 304
            if ((i >= ((*tok).start + (*tok).len))) {
#line 306
                if ((out <= limit)) {
                    buf[out] = '\0';
                }
#line 307
                return 0;
            }
#line 309
            esc = (*(*p).src)[i];
#line 310
            if ((esc == 'n')) {
#line 311
                if ((out <= limit)) {
                    buf[out] = ((char)(10));
                    (out++);
                }
            } else if ((esc == 't')) {
#line 313
                if ((out <= limit)) {
                    buf[out] = ((char)(9));
                    (out++);
                }
            } else if ((esc == 'r')) {
#line 315
                if ((out <= limit)) {
                    buf[out] = ((char)(13));
                    (out++);
                }
            } else if ((esc == 'b')) {
#line 317
                if ((out <= limit)) {
                    buf[out] = ((char)(8));
                    (out++);
                }
            } else if ((esc == 'f')) {
#line 319
                if ((out <= limit)) {
                    buf[out] = ((char)(12));
                    (out++);
                }
            } else if ((esc == ((char)(92)))) {
#line 321
                if ((out <= limit)) {
                    buf[out] = ((char)(92));
                    (out++);
                }
            } else if ((esc == '"')) {
#line 323
                if ((out <= limit)) {
                    buf[out] = '"';
                    (out++);
                }
            } else if ((esc == '/')) {
#line 325
                if ((out <= limit)) {
                    buf[out] = '/';
                    (out++);
                }
            } else if ((esc == 'u')) {
#line 328
                cp = 0;
#line 329
                h = 0;
#line 330
                while (((h < 4) && (((i + 1) + h) < ((*tok).start + (*tok).len)))) {
#line 331
                    if (HexVal((*(*p).src)[((i + 1) + h)], &hv)) {
#line 332
                        cp = ((cp * 16) + hv);
                    } else {
#line 334
                        if ((out <= limit)) {
                            buf[out] = '\0';
                        }
#line 335
                        return 0;
                    }
#line 337
                    (h++);
                }
#line 339
                if ((h < 4)) {
#line 340
                    if ((out <= limit)) {
                        buf[out] = '\0';
                    }
#line 341
                    return 0;
                }
#line 343
                i = (i + 4);
#line 345
                if ((cp < 128)) {
#line 346
                    if ((out <= limit)) {
                        buf[out] = ((char)(cp));
                        (out++);
                    }
                } else if ((cp < 2048)) {
#line 348
                    if ((out <= limit)) {
#line 349
                        buf[out] = ((char)((192 + ((uint32_t)(cp) / (uint32_t)(64)))));
                        (out++);
                    }
#line 351
                    if ((out <= limit)) {
#line 352
                        buf[out] = ((char)((128 + ((uint32_t)(cp) % (uint32_t)(64)))));
                        (out++);
                    }
                } else {
#line 355
                    if ((out <= limit)) {
#line 356
                        buf[out] = ((char)((224 + ((uint32_t)(cp) / (uint32_t)(4096)))));
                        (out++);
                    }
#line 358
                    if ((out <= limit)) {
#line 359
                        buf[out] = ((char)((128 + ((uint32_t)(((uint32_t)(cp) / (uint32_t)(64))) % (uint32_t)(64)))));
                        (out++);
                    }
#line 361
                    if ((out <= limit)) {
#line 362
                        buf[out] = ((char)((128 + ((uint32_t)(cp) % (uint32_t)(64)))));
                        (out++);
                    }
                }
            } else {
#line 367
                if ((out <= limit)) {
                    buf[out] = esc;
                    (out++);
                }
            }
#line 369
            (i++);
        } else {
#line 371
            if ((out <= limit)) {
                buf[out] = ch;
                (out++);
            }
#line 372
            (i++);
        }
    }
#line 376
    if ((out <= limit)) {
        buf[out] = '\0';
    }
#line 377
    return 1;
}

#line 380
int GetInteger(Parser *p, Token *tok, int32_t *val) {
    uint32_t i, j, endPos;
    int neg;
    char ch;
    int32_t result;
#line 388
    if (((*tok).kind != TokenKind_JNumber)) {
        return 0;
    }
#line 390
    i = (*tok).start;
#line 391
    endPos = ((*tok).start + (*tok).len);
#line 392
    neg = 0;
#line 393
    result = 0;
#line 395
    if (((i < endPos) && ((*(*p).src)[i] == '-'))) {
#line 396
        neg = 1;
#line 397
        (i++);
    }
#line 401
    j = i;
#line 402
    while ((j < endPos)) {
#line 403
        ch = (*(*p).src)[j];
#line 404
        if ((((ch == '.') || (ch == 'e')) || (ch == 'E'))) {
#line 405
            return 0;
        }
#line 407
        (j++);
    }
#line 410
    while ((i < endPos)) {
#line 411
        ch = (*(*p).src)[i];
#line 412
        if ((!IsDigit(ch))) {
            return 0;
        }
#line 413
        result = ((result * 10) + ((int32_t)((((uint32_t)((unsigned char)(ch))) - ((uint32_t)((unsigned char)('0')))))));
#line 414
        (i++);
    }
#line 417
    if (neg) {
        (*val) = (-result);
    } else {
        (*val) = result;
    }
#line 418
    return 1;
}

#line 421
int GetReal(Parser *p, Token *tok, float *val) {
    uint32_t i, endPos;
    int neg, negExp;
    char ch;
    float result, frac, divisor;
    int32_t exp;
#line 430
    if (((*tok).kind != TokenKind_JNumber)) {
        return 0;
    }
#line 432
    i = (*tok).start;
#line 433
    endPos = ((*tok).start + (*tok).len);
#line 434
    neg = 0;
#line 435
    result = 0.0;
#line 437
    if (((i < endPos) && ((*(*p).src)[i] == '-'))) {
#line 438
        neg = 1;
#line 439
        (i++);
    }
#line 443
    while (((i < endPos) && IsDigit((*(*p).src)[i]))) {
#line 444
        result = ((result * 10.0) + ((float)((((uint32_t)((unsigned char)((*(*p).src)[i]))) - ((uint32_t)((unsigned char)('0')))))));
#line 445
        (i++);
    }
#line 449
    if (((i < endPos) && ((*(*p).src)[i] == '.'))) {
#line 450
        (i++);
#line 451
        divisor = 10.0;
#line 452
        while (((i < endPos) && IsDigit((*(*p).src)[i]))) {
#line 453
            frac = ((float)((((uint32_t)((unsigned char)((*(*p).src)[i]))) - ((uint32_t)((unsigned char)('0'))))));
#line 454
            result = (result + ((double)(frac) / (double)(divisor)));
#line 455
            divisor = (divisor * 10.0);
#line 456
            (i++);
        }
    }
#line 461
    if (((i < endPos) && (((*(*p).src)[i] == 'e') || ((*(*p).src)[i] == 'E')))) {
#line 462
        (i++);
#line 463
        negExp = 0;
#line 464
        if (((i < endPos) && ((*(*p).src)[i] == '-'))) {
#line 465
            negExp = 1;
            (i++);
        } else if (((i < endPos) && ((*(*p).src)[i] == '+'))) {
#line 467
            (i++);
        }
#line 469
        exp = 0;
#line 470
        while (((i < endPos) && IsDigit((*(*p).src)[i]))) {
#line 471
            exp = ((exp * 10) + ((int32_t)((((uint32_t)((unsigned char)((*(*p).src)[i]))) - ((uint32_t)((unsigned char)('0')))))));
#line 472
            (i++);
        }
#line 475
        while ((exp > 0)) {
#line 476
            if (negExp) {
                result = ((double)(result) / (double)(10.0));
            } else {
#line 477
                result = (result * 10.0);
            }
#line 479
            (exp--);
        }
    }
#line 483
    if (neg) {
        (*val) = (-result);
    } else {
        (*val) = result;
    }
#line 484
    return 1;
}

#line 489
void Skip(Parser *p) {
    Token tok;
    int32_t depth;
#line 492
    if ((!Next(p, &tok))) {
        return;
    }
#line 494
    if ((tok.kind == TokenKind_JObjectStart)) {
#line 495
        depth = 1;
#line 496
        while (((depth > 0) && Next(p, &tok))) {
#line 497
            if ((tok.kind == TokenKind_JObjectStart)) {
                (depth++);
            } else if ((tok.kind == TokenKind_JObjectEnd)) {
#line 498
                (depth--);
            }
        }
    } else if ((tok.kind == TokenKind_JArrayStart)) {
#line 502
        depth = 1;
#line 503
        while (((depth > 0) && Next(p, &tok))) {
#line 504
            if ((tok.kind == TokenKind_JArrayStart)) {
                (depth++);
            } else if ((tok.kind == TokenKind_JArrayEnd)) {
#line 505
                (depth--);
            }
        }
    }
}

#line 514
void GetError(Parser *p, char *buf, uint32_t buf_high) {
#line 516
    if ((*p).hasError) {
#line 517
        CopyStr((*p).err, (sizeof((*p).err) / sizeof((*p).err[0])) - 1, buf, buf_high);
    } else {
#line 519
        if ((0 <= buf_high)) {
            buf[0] = '\0';
        }
    }
}
