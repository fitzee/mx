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

/* Foreign C bindings: RegexBridge */
extern void * m2_regex_compile(void * pattern);
extern void m2_regex_free(void * re);
extern int32_t m2_regex_test(void * re, void * text);
extern int32_t m2_regex_find(void * re, void * text, int32_t *start, int32_t *len);
extern int32_t m2_regex_find_all(void * re, void * text, void * starts, void * lens, int32_t maxMatches, int32_t *count);
extern void m2_regex_error(void * buf, int32_t bufLen);

/* Imported Module Regex */

typedef struct Regex_Match Regex_Match;
static const int32_t Regex_MaxMatches = 32;
static const int32_t Regex_MaxErrorLen = 256;
typedef void * Regex_Regex;

struct Regex_Match {
    uint32_t start;
    uint32_t len;
};

typedef enum { Regex_Status_Ok, Regex_Status_NoMatch, Regex_Status_BadPattern, Regex_Status_Error } Regex_Status;
#define m2_min_Regex_Status 0
#define m2_max_Regex_Status 3

static Regex_Status Regex_Compile(char *pattern, uint32_t pattern_high, Regex_Regex *re);
static void Regex_Free(Regex_Regex *re);
static int Regex_Test(Regex_Regex re, char *text, uint32_t text_high);
static Regex_Status Regex_Find(Regex_Regex re, char *text, uint32_t text_high, Regex_Match *m);
static Regex_Status Regex_FindAll(Regex_Regex re, char *text, uint32_t text_high, Regex_Match *matches, uint32_t matches_high, uint32_t maxMatches, uint32_t *count);
static void Regex_GetError(char *buf, uint32_t buf_high);

static Regex_Status Regex_Compile(char *pattern, uint32_t pattern_high, Regex_Regex *re) {
    void * p;
#line 13 "libs/m2regex/src/Regex.mod"
    p = m2_regex_compile(((void *)(pattern)));
#line 14
    if ((p == NULL)) {
#line 15
        (*re) = NULL;
#line 16
        return Regex_Status_BadPattern;
    }
#line 18
    (*re) = p;
#line 19
    return Regex_Status_Ok;
}

static void Regex_Free(Regex_Regex *re) {
#line 24
    if (((*re) != NULL)) {
#line 25
        m2_regex_free((*re));
#line 26
        (*re) = NULL;
    }
}

static int Regex_Test(Regex_Regex re, char *text, uint32_t text_high) {
#line 34
    if ((re == NULL)) {
        return 0;
    }
#line 35
    return (m2_regex_test(re, ((void *)(text))) == 1);
}

static Regex_Status Regex_Find(Regex_Regex re, char *text, uint32_t text_high, Regex_Match *m) {
    int32_t rc;
    int32_t s, l;
#line 43
    if ((re == NULL)) {
        return Regex_Status_Error;
    }
#line 44
    rc = m2_regex_find(re, ((void *)(text)), &s, &l);
#line 45
    if ((rc == 0)) {
#line 46
        (*m).start = ((uint32_t)(s));
#line 47
        (*m).len = ((uint32_t)(l));
#line 48
        return Regex_Status_Ok;
    } else if ((rc == 1)) {
#line 50
        return Regex_Status_NoMatch;
    } else {
#line 52
        return Regex_Status_Error;
    }
}

static Regex_Status Regex_FindAll(Regex_Regex re, char *text, uint32_t text_high, Regex_Match *matches, uint32_t matches_high, uint32_t maxMatches, uint32_t *count) {
    int32_t rc;
    uint32_t i;
    int32_t max;
    int32_t cnt;
    int32_t starts[31 + 1];
    int32_t lens[31 + 1];
#line 67
    if ((re == NULL)) {
        return Regex_Status_Error;
    }
#line 68
    (*count) = 0;
#line 71
    if ((maxMatches > Regex_MaxMatches)) {
#line 72
        max = Regex_MaxMatches;
    } else {
#line 74
        max = ((int32_t)(maxMatches));
    }
#line 77
    rc = m2_regex_find_all(re, ((void *)(text)), ((void *)&(starts)), ((void *)&(lens)), max, &cnt);
#line 80
    if ((rc == 0)) {
#line 81
        (*count) = ((uint32_t)(cnt));
#line 82
        i = 0;
#line 83
        while ((i < (*count))) {
#line 84
            matches[i].start = ((uint32_t)(starts[i]));
#line 85
            matches[i].len = ((uint32_t)(lens[i]));
#line 86
            (i++);
        }
#line 88
        return Regex_Status_Ok;
    } else if ((rc == 1)) {
#line 90
        return Regex_Status_NoMatch;
    } else {
#line 92
        return Regex_Status_Error;
    }
}

static void Regex_GetError(char *buf, uint32_t buf_high) {
#line 100
    m2_regex_error(((void *)(buf)), (buf_high + 1));
}

/* Module RegexTests */

void Check(char *name, uint32_t name_high, int cond);
void TestCompileOk(void);
void TestMatch(void);
void TestNoMatch(void);
void TestFindBasic(void);
void TestFindOffset(void);
void TestFindNoMatch(void);
void TestFindAllMulti(void);
void TestFindAllSingle(void);
void TestFindAllNone(void);
void TestBadPattern(void);
void TestErrorMessage(void);
void TestFreeNil(void);
void TestNilRegex(void);
void TestDigitClass(void);
void TestAlternation(void);

int32_t passed, failed, total;

#line 29 "libs/m2regex/tests/regex_tests.mod"
void Check(char *name, uint32_t name_high, int cond) {
#line 31
    (total++);
#line 32
    if (cond) {
#line 33
        (passed++);
    } else {
#line 35
        (failed++);
#line 36
        m2_WriteString("FAIL: ");
        m2_WriteString(name);
        m2_WriteLn();
    }
}

#line 42
void TestCompileOk(void) {
    Regex_Regex re;
    Regex_Status s;
#line 45
    s = Regex_Compile("hello", (sizeof("hello") / sizeof("hello"[0])) - 1, &re);
#line 46
    Check("compile: ok status", (sizeof("compile: ok status") / sizeof("compile: ok status"[0])) - 1, (s == Regex_Status_Ok));
#line 47
    Check("compile: re not nil", (sizeof("compile: re not nil") / sizeof("compile: re not nil"[0])) - 1, (re != NULL));
#line 48
    Regex_Free(&re);
}

#line 53
void TestMatch(void) {
    Regex_Regex re;
    Regex_Status s;
#line 56
    s = Regex_Compile("world", (sizeof("world") / sizeof("world"[0])) - 1, &re);
#line 57
    Check("match: compile ok", (sizeof("match: compile ok") / sizeof("match: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 58
    Check("match: hello world", (sizeof("match: hello world") / sizeof("match: hello world"[0])) - 1, Regex_Test(re, "hello world", (sizeof("hello world") / sizeof("hello world"[0])) - 1));
#line 59
    Check("match: world alone", (sizeof("match: world alone") / sizeof("match: world alone"[0])) - 1, Regex_Test(re, "world", (sizeof("world") / sizeof("world"[0])) - 1));
#line 60
    Regex_Free(&re);
}

#line 65
void TestNoMatch(void) {
    Regex_Regex re;
    Regex_Status s;
#line 68
    s = Regex_Compile("xyz", (sizeof("xyz") / sizeof("xyz"[0])) - 1, &re);
#line 69
    Check("nomatch: compile ok", (sizeof("nomatch: compile ok") / sizeof("nomatch: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 70
    Check("nomatch: abc", (sizeof("nomatch: abc") / sizeof("nomatch: abc"[0])) - 1, (!Regex_Test(re, "abc", (sizeof("abc") / sizeof("abc"[0])) - 1)));
#line 71
    Check("nomatch: empty", (sizeof("nomatch: empty") / sizeof("nomatch: empty"[0])) - 1, (!Regex_Test(re, "", (sizeof("") / sizeof(""[0])) - 1)));
#line 72
    Regex_Free(&re);
}

#line 77
void TestFindBasic(void) {
    Regex_Regex re;
    Regex_Match m;
    Regex_Status s;
#line 80
    s = Regex_Compile("foo", (sizeof("foo") / sizeof("foo"[0])) - 1, &re);
#line 81
    Check("find: compile ok", (sizeof("find: compile ok") / sizeof("find: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 82
    s = Regex_Find(re, "foo bar", (sizeof("foo bar") / sizeof("foo bar"[0])) - 1, &m);
#line 83
    Check("find: status ok", (sizeof("find: status ok") / sizeof("find: status ok"[0])) - 1, (s == Regex_Status_Ok));
#line 84
    Check("find: start=0", (sizeof("find: start=0") / sizeof("find: start=0"[0])) - 1, (m.start == 0));
#line 85
    Check("find: len=3", (sizeof("find: len=3") / sizeof("find: len=3"[0])) - 1, (m.len == 3));
#line 86
    Regex_Free(&re);
}

#line 91
void TestFindOffset(void) {
    Regex_Regex re;
    Regex_Match m;
    Regex_Status s;
#line 94
    s = Regex_Compile("[0-9]+", (sizeof("[0-9]+") / sizeof("[0-9]+"[0])) - 1, &re);
#line 95
    Check("offset: compile ok", (sizeof("offset: compile ok") / sizeof("offset: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 96
    s = Regex_Find(re, "abc 42 def", (sizeof("abc 42 def") / sizeof("abc 42 def"[0])) - 1, &m);
#line 97
    Check("offset: status ok", (sizeof("offset: status ok") / sizeof("offset: status ok"[0])) - 1, (s == Regex_Status_Ok));
#line 98
    Check("offset: start=4", (sizeof("offset: start=4") / sizeof("offset: start=4"[0])) - 1, (m.start == 4));
#line 99
    Check("offset: len=2", (sizeof("offset: len=2") / sizeof("offset: len=2"[0])) - 1, (m.len == 2));
#line 100
    Regex_Free(&re);
}

#line 105
void TestFindNoMatch(void) {
    Regex_Regex re;
    Regex_Match m;
    Regex_Status s;
#line 108
    s = Regex_Compile("[0-9]+", (sizeof("[0-9]+") / sizeof("[0-9]+"[0])) - 1, &re);
#line 109
    Check("findnm: compile ok", (sizeof("findnm: compile ok") / sizeof("findnm: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 110
    s = Regex_Find(re, "no digits here", (sizeof("no digits here") / sizeof("no digits here"[0])) - 1, &m);
#line 111
    Check("findnm: NoMatch", (sizeof("findnm: NoMatch") / sizeof("findnm: NoMatch"[0])) - 1, (s == Regex_Status_NoMatch));
#line 112
    Regex_Free(&re);
}

#line 117
void TestFindAllMulti(void) {
    Regex_Regex re;
    Regex_Match ms[31 + 1];
    uint32_t count;
    Regex_Status s;
#line 124
    s = Regex_Compile("[0-9]+", (sizeof("[0-9]+") / sizeof("[0-9]+"[0])) - 1, &re);
#line 125
    Check("findall: compile ok", (sizeof("findall: compile ok") / sizeof("findall: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 126
    s = Regex_FindAll(re, "a1b22c333d", (sizeof("a1b22c333d") / sizeof("a1b22c333d"[0])) - 1, ms, (sizeof(ms) / sizeof(ms[0])) - 1, Regex_MaxMatches, &count);
#line 127
    Check("findall: status ok", (sizeof("findall: status ok") / sizeof("findall: status ok"[0])) - 1, (s == Regex_Status_Ok));
#line 128
    Check("findall: count=3", (sizeof("findall: count=3") / sizeof("findall: count=3"[0])) - 1, (count == 3));
#line 131
    Check("findall: m0 start=1", (sizeof("findall: m0 start=1") / sizeof("findall: m0 start=1"[0])) - 1, (ms[0].start == 1));
#line 132
    Check("findall: m0 len=1", (sizeof("findall: m0 len=1") / sizeof("findall: m0 len=1"[0])) - 1, (ms[0].len == 1));
#line 135
    Check("findall: m1 start=3", (sizeof("findall: m1 start=3") / sizeof("findall: m1 start=3"[0])) - 1, (ms[1].start == 3));
#line 136
    Check("findall: m1 len=2", (sizeof("findall: m1 len=2") / sizeof("findall: m1 len=2"[0])) - 1, (ms[1].len == 2));
#line 139
    Check("findall: m2 start=6", (sizeof("findall: m2 start=6") / sizeof("findall: m2 start=6"[0])) - 1, (ms[2].start == 6));
#line 140
    Check("findall: m2 len=3", (sizeof("findall: m2 len=3") / sizeof("findall: m2 len=3"[0])) - 1, (ms[2].len == 3));
#line 141
    Regex_Free(&re);
}

#line 146
void TestFindAllSingle(void) {
    Regex_Regex re;
    Regex_Match ms[31 + 1];
    uint32_t count;
    Regex_Status s;
#line 153
    s = Regex_Compile("only", (sizeof("only") / sizeof("only"[0])) - 1, &re);
#line 154
    Check("findall1: compile ok", (sizeof("findall1: compile ok") / sizeof("findall1: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 155
    s = Regex_FindAll(re, "the only one", (sizeof("the only one") / sizeof("the only one"[0])) - 1, ms, (sizeof(ms) / sizeof(ms[0])) - 1, Regex_MaxMatches, &count);
#line 156
    Check("findall1: status ok", (sizeof("findall1: status ok") / sizeof("findall1: status ok"[0])) - 1, (s == Regex_Status_Ok));
#line 157
    Check("findall1: count=1", (sizeof("findall1: count=1") / sizeof("findall1: count=1"[0])) - 1, (count == 1));
#line 158
    Check("findall1: start=4", (sizeof("findall1: start=4") / sizeof("findall1: start=4"[0])) - 1, (ms[0].start == 4));
#line 159
    Check("findall1: len=4", (sizeof("findall1: len=4") / sizeof("findall1: len=4"[0])) - 1, (ms[0].len == 4));
#line 160
    Regex_Free(&re);
}

#line 165
void TestFindAllNone(void) {
    Regex_Regex re;
    Regex_Match ms[31 + 1];
    uint32_t count;
    Regex_Status s;
#line 172
    s = Regex_Compile("zzz", (sizeof("zzz") / sizeof("zzz"[0])) - 1, &re);
#line 173
    Check("findall0: compile ok", (sizeof("findall0: compile ok") / sizeof("findall0: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 174
    s = Regex_FindAll(re, "no match here", (sizeof("no match here") / sizeof("no match here"[0])) - 1, ms, (sizeof(ms) / sizeof(ms[0])) - 1, Regex_MaxMatches, &count);
#line 175
    Check("findall0: NoMatch", (sizeof("findall0: NoMatch") / sizeof("findall0: NoMatch"[0])) - 1, (s == Regex_Status_NoMatch));
#line 176
    Check("findall0: count=0", (sizeof("findall0: count=0") / sizeof("findall0: count=0"[0])) - 1, (count == 0));
#line 177
    Regex_Free(&re);
}

#line 182
void TestBadPattern(void) {
    Regex_Regex re;
    Regex_Status s;
#line 185
    s = Regex_Compile("[invalid", (sizeof("[invalid") / sizeof("[invalid"[0])) - 1, &re);
#line 186
    Check("badpat: BadPattern", (sizeof("badpat: BadPattern") / sizeof("badpat: BadPattern"[0])) - 1, (s == Regex_Status_BadPattern));
#line 187
    Check("badpat: re is nil", (sizeof("badpat: re is nil") / sizeof("badpat: re is nil"[0])) - 1, (re == NULL));
}

#line 192
void TestErrorMessage(void) {
    Regex_Regex re;
    Regex_Status s;
    char buf[255 + 1];
#line 198
    s = Regex_Compile("[bad", (sizeof("[bad") / sizeof("[bad"[0])) - 1, &re);
#line 199
    Check("errmsg: BadPattern", (sizeof("errmsg: BadPattern") / sizeof("errmsg: BadPattern"[0])) - 1, (s == Regex_Status_BadPattern));
#line 200
    Regex_GetError(buf, (sizeof(buf) / sizeof(buf[0])) - 1);
#line 202
    Check("errmsg: non-empty", (sizeof("errmsg: non-empty") / sizeof("errmsg: non-empty"[0])) - 1, (buf[0] != '\0'));
}

#line 207
void TestFreeNil(void) {
    Regex_Regex re;
#line 210
    re = NULL;
#line 211
    Regex_Free(&re);
#line 212
    Check("freenil: no crash", (sizeof("freenil: no crash") / sizeof("freenil: no crash"[0])) - 1, 1);
}

#line 217
void TestNilRegex(void) {
    Regex_Regex re;
#line 220
    re = NULL;
#line 221
    Check("testnil: returns false", (sizeof("testnil: returns false") / sizeof("testnil: returns false"[0])) - 1, (!Regex_Test(re, "anything", (sizeof("anything") / sizeof("anything"[0])) - 1)));
}

#line 226
void TestDigitClass(void) {
    Regex_Regex re;
    Regex_Match m;
    Regex_Status s;
#line 229
    s = Regex_Compile("[[:digit:]]+", (sizeof("[[:digit:]]+") / sizeof("[[:digit:]]+"[0])) - 1, &re);
#line 230
    Check("digit: compile ok", (sizeof("digit: compile ok") / sizeof("digit: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 231
    Check("digit: matches 123", (sizeof("digit: matches 123") / sizeof("digit: matches 123"[0])) - 1, Regex_Test(re, "abc123def", (sizeof("abc123def") / sizeof("abc123def"[0])) - 1));
#line 232
    Check("digit: no match letters", (sizeof("digit: no match letters") / sizeof("digit: no match letters"[0])) - 1, (!Regex_Test(re, "abcdef", (sizeof("abcdef") / sizeof("abcdef"[0])) - 1)));
#line 233
    s = Regex_Find(re, "abc123def", (sizeof("abc123def") / sizeof("abc123def"[0])) - 1, &m);
#line 234
    Check("digit: find ok", (sizeof("digit: find ok") / sizeof("digit: find ok"[0])) - 1, (s == Regex_Status_Ok));
#line 235
    Check("digit: start=3", (sizeof("digit: start=3") / sizeof("digit: start=3"[0])) - 1, (m.start == 3));
#line 236
    Check("digit: len=3", (sizeof("digit: len=3") / sizeof("digit: len=3"[0])) - 1, (m.len == 3));
#line 237
    Regex_Free(&re);
}

#line 242
void TestAlternation(void) {
    Regex_Regex re;
    Regex_Status s;
#line 245
    s = Regex_Compile("cat|dog", (sizeof("cat|dog") / sizeof("cat|dog"[0])) - 1, &re);
#line 246
    Check("alt: compile ok", (sizeof("alt: compile ok") / sizeof("alt: compile ok"[0])) - 1, (s == Regex_Status_Ok));
#line 247
    Check("alt: matches cat", (sizeof("alt: matches cat") / sizeof("alt: matches cat"[0])) - 1, Regex_Test(re, "I have a cat", (sizeof("I have a cat") / sizeof("I have a cat"[0])) - 1));
#line 248
    Check("alt: matches dog", (sizeof("alt: matches dog") / sizeof("alt: matches dog"[0])) - 1, Regex_Test(re, "I have a dog", (sizeof("I have a dog") / sizeof("I have a dog"[0])) - 1));
#line 249
    Check("alt: no match fish", (sizeof("alt: no match fish") / sizeof("alt: no match fish"[0])) - 1, (!Regex_Test(re, "I have a fish", (sizeof("I have a fish") / sizeof("I have a fish"[0])) - 1)));
#line 250
    Regex_Free(&re);
}
int main(int _m2_argc, char **_m2_argv) {
    m2_argc = _m2_argc; m2_argv = _m2_argv;
#line 26
#line 256
    passed = 0;
#line 257
    failed = 0;
#line 258
    total = 0;
#line 260
    m2_WriteString("m2regex test suite");
    m2_WriteLn();
#line 261
    m2_WriteString("==================");
    m2_WriteLn();
#line 263
    TestCompileOk();
#line 264
    TestMatch();
#line 265
    TestNoMatch();
#line 266
    TestFindBasic();
#line 267
    TestFindOffset();
#line 268
    TestFindNoMatch();
#line 269
    TestFindAllMulti();
#line 270
    TestFindAllSingle();
#line 271
    TestFindAllNone();
#line 272
    TestBadPattern();
#line 273
    TestErrorMessage();
#line 274
    TestFreeNil();
#line 275
    TestNilRegex();
#line 276
    TestDigitClass();
#line 277
    TestAlternation();
#line 279
    m2_WriteLn();
#line 280
    m2_WriteString("m2regex: ");
#line 281
    m2_WriteInt(passed, 0);
    m2_WriteString(" passed, ");
#line 282
    m2_WriteInt(failed, 0);
    m2_WriteString(" failed, ");
#line 283
    m2_WriteInt(total, 0);
    m2_WriteString(" total");
    m2_WriteLn();
#line 285
    if ((failed > 0)) {
#line 286
        m2_WriteString("*** FAILURES ***");
        m2_WriteLn();
    } else {
#line 288
        m2_WriteString("*** ALL TESTS PASSED ***");
        m2_WriteLn();
    }
    return 0;
}
