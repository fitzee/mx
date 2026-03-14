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

/* Foreign C bindings: PollerBridge */
extern int32_t m2_poller_create(void);
extern void m2_poller_destroy(int32_t handle);
extern int32_t m2_poller_add(int32_t handle, int32_t fd, int32_t events);
extern int32_t m2_poller_mod(int32_t handle, int32_t fd, int32_t events);
extern int32_t m2_poller_del(int32_t handle, int32_t fd);
extern int32_t m2_poller_wait(int32_t handle, int32_t timeoutMs, void * out, int32_t maxEvents);
extern int32_t m2_now_ms(void);

/* Imported Module Scheduler */

typedef struct Scheduler_TaskNode Scheduler_TaskNode;
typedef struct Scheduler_SchedulerRec Scheduler_SchedulerRec;
typedef enum { Scheduler_Status_OK, Scheduler_Status_Invalid, Scheduler_Status_OutOfMemory, Scheduler_Status_AlreadySettled } Scheduler_Status;
#define m2_min_Scheduler_Status 0
#define m2_max_Scheduler_Status 3

typedef void (*Scheduler_TaskProc)(void *);

typedef void * Scheduler_Scheduler;

static const int32_t Scheduler_MAXQ = 4096;
struct Scheduler_TaskNode {
    Scheduler_TaskProc cb;
    void * data;
};

typedef Scheduler_SchedulerRec *Scheduler_SchedulerPtr;

struct Scheduler_SchedulerRec {
    Scheduler_TaskNode q[4095 + 1];
    uint32_t cap;
    uint32_t head;
    uint32_t tail;
    uint32_t count;
};

static Scheduler_Status Scheduler_SchedulerCreate(uint32_t capacity, Scheduler_Scheduler *out);
static Scheduler_Status Scheduler_SchedulerDestroy(Scheduler_Scheduler *s);
static Scheduler_Status Scheduler_SchedulerEnqueue(Scheduler_Scheduler s, Scheduler_TaskProc cb, void * user);
static Scheduler_Status Scheduler_SchedulerPump(Scheduler_Scheduler s, uint32_t maxSteps, int *didWork);

static Scheduler_Status Scheduler_SchedulerCreate(uint32_t capacity, Scheduler_Scheduler *out) {
    Scheduler_SchedulerPtr sp;
#line 28 "/Users/mattfitz/.mx/lib/m2futures/src/Scheduler.mod"
    if ((capacity == 0)) {
#line 29
        (*out) = NULL;
#line 30
        return Scheduler_Status_Invalid;
    }
#line 32
    if ((capacity > Scheduler_MAXQ)) {
#line 33
        capacity = Scheduler_MAXQ;
    }
#line 35
    sp = GC_MALLOC(sizeof(*sp));
#line 36
    if ((sp == NULL)) {
#line 37
        (*out) = NULL;
#line 38
        return Scheduler_Status_OutOfMemory;
    }
#line 40
    sp->cap = capacity;
#line 41
    sp->head = 0;
#line 42
    sp->tail = 0;
#line 43
    sp->count = 0;
#line 44
    (*out) = sp;
#line 45
    return Scheduler_Status_OK;
}

static Scheduler_Status Scheduler_SchedulerDestroy(Scheduler_Scheduler *s) {
    Scheduler_SchedulerPtr sp;
#line 51
    if (((*s) == NULL)) {
        return Scheduler_Status_Invalid;
    }
#line 52
    sp = (*s);
#line 53
    GC_FREE(sp);
#line 54
    (*s) = NULL;
#line 55
    return Scheduler_Status_OK;
}

static Scheduler_Status Scheduler_SchedulerEnqueue(Scheduler_Scheduler s, Scheduler_TaskProc cb, void * user) {
    Scheduler_SchedulerPtr sp;
#line 63
    if ((s == NULL)) {
        return Scheduler_Status_Invalid;
    }
#line 64
    sp = s;
#line 65
    if ((sp->count >= sp->cap)) {
        return Scheduler_Status_OutOfMemory;
    }
#line 66
    sp->q[sp->tail].cb = cb;
#line 67
    sp->q[sp->tail].data = user;
#line 68
    sp->tail = m2_mod((sp->tail + 1), sp->cap);
#line 69
    sp->count = (sp->count + 1);
#line 70
    return Scheduler_Status_OK;
}

static Scheduler_Status Scheduler_SchedulerPump(Scheduler_Scheduler s, uint32_t maxSteps, int *didWork) {
    Scheduler_SchedulerPtr sp;
    uint32_t steps;
    Scheduler_TaskProc fn;
    void * arg;
#line 82
    if ((s == NULL)) {
#line 83
        (*didWork) = 0;
#line 84
        return Scheduler_Status_Invalid;
    }
#line 86
    sp = s;
#line 87
    (*didWork) = 0;
#line 88
    steps = 0;
#line 89
    while (((steps < maxSteps) && (sp->count > 0))) {
#line 90
        fn = sp->q[sp->head].cb;
#line 91
        arg = sp->q[sp->head].data;
#line 92
        sp->head = m2_mod((sp->head + 1), sp->cap);
#line 93
        sp->count = (sp->count - 1);
#line 94
        fn(arg);
#line 95
        (*didWork) = 1;
#line 96
        steps = (steps + 1);
    }
#line 98
    return Scheduler_Status_OK;
}

/* Imported Module Promise */

typedef struct Promise_Value Promise_Value;
typedef struct Promise_Error Promise_Error;
typedef struct Promise_Result Promise_Result;
typedef struct Promise_SharedRec Promise_SharedRec;
typedef struct Promise_ContRec Promise_ContRec;
typedef struct Promise_AllStateRec Promise_AllStateRec;
typedef struct Promise_RaceStateRec Promise_RaceStateRec;
typedef struct Promise_CancelCB Promise_CancelCB;
typedef struct Promise_CancelRec Promise_CancelRec;
typedef struct Promise_CancMapRec Promise_CancMapRec;
typedef enum { Promise_Fate_Pending, Promise_Fate_Fulfilled, Promise_Fate_Rejected } Promise_Fate;
#define m2_min_Promise_Fate 0
#define m2_max_Promise_Fate 2

struct Promise_Value {
    int32_t tag;
    void * ptr;
};

struct Promise_Error {
    int32_t code;
    void * ptr;
};

struct Promise_Result {
    int isOk;
    Promise_Value v;
    Promise_Error e;
};

typedef void (*Promise_ThenFn)(Promise_Result, void *, Promise_Result *);

typedef void (*Promise_CatchFn)(Promise_Error, void *, Promise_Result *);

typedef void (*Promise_VoidFn)(Promise_Result, void *);

typedef void * Promise_Promise;

typedef void * Promise_Future;

static const int32_t Promise_MAX_ALL_SIZE = 32;
typedef Promise_Result Promise_AllResultArray[31 + 1];

typedef Promise_AllResultArray *Promise_AllResultPtr;

typedef void * Promise_CancelToken;

static const int32_t Promise_POOL_SH = 256;
static const int32_t Promise_POOL_CN = 512;
typedef enum { Promise_ContKind_CKThen, Promise_ContKind_CKCatch, Promise_ContKind_CKFinally, Promise_ContKind_CKAll, Promise_ContKind_CKRace } Promise_ContKind;
#define m2_min_Promise_ContKind 0
#define m2_max_Promise_ContKind 4

typedef Promise_SharedRec *Promise_SharedPtr;

typedef Promise_ContRec *Promise_ContPtr;

struct Promise_SharedRec {
    Scheduler_Scheduler sched;
    Promise_Fate fate;
    Promise_Result res;
    Promise_ContPtr contHead;
    Promise_ContPtr contTail;
    uint32_t poolIdx;
};

struct Promise_ContRec {
    Promise_ContKind kind;
    Promise_ThenFn thenFn;
    Promise_CatchFn catchFn;
    Promise_VoidFn voidFn;
    void * user;
    Promise_SharedPtr inSh;
    Promise_SharedPtr outSh;
    void * combSt;
    uint32_t idx;
    Promise_ContPtr next;
    uint32_t poolIdx;
};

typedef Promise_AllStateRec *Promise_AllStatePtr;

struct Promise_AllStateRec {
    Promise_SharedPtr outSh;
    uint32_t total;
    uint32_t done;
    int failed;
    Promise_AllResultArray results;
};

typedef Promise_RaceStateRec *Promise_RaceStatePtr;

struct Promise_RaceStateRec {
    Promise_SharedPtr outSh;
    int settled;
};

static const int32_t Promise_POOL_CT = 64;
static const int32_t Promise_MaxCancelCBs = 8;
struct Promise_CancelCB {
    Promise_VoidFn fn;
    void * ctx;
};

struct Promise_CancelRec {
    int cancelled;
    Scheduler_Scheduler sched;
    Promise_CancelCB cbs[7 + 1];
    int32_t cbCount;
    uint32_t poolIdx;
};

typedef Promise_CancelRec *Promise_CancelPtr;

struct Promise_CancMapRec {
    Promise_ThenFn fn;
    void * user;
    Promise_CancelToken ct;
};

typedef Promise_CancMapRec *Promise_CancMapPtr;

static void Promise_InitPools(void);
static int Promise_AllocShared(Promise_SharedPtr *p);
static void Promise_FreeShared(Promise_SharedPtr p);
static int Promise_AllocCont(Promise_ContPtr *c);
static void Promise_FreeCont(Promise_ContPtr c);
static void Promise_AppendCont(Promise_SharedPtr sh, Promise_ContPtr c);
static void Promise_DrainConts(Promise_SharedPtr sh);
static void Promise_SettleWith(Promise_SharedPtr sh, Promise_Result *res);
static void Promise_HandleAll(Promise_ContPtr c);
static void Promise_HandleRace(Promise_ContPtr c);
static void Promise_ExecuteCont(void * data);
static Scheduler_Status Promise_PromiseCreate(Scheduler_Scheduler s, Promise_Promise *p, Promise_Future *f);
static Scheduler_Status Promise_Resolve(Promise_Promise p, Promise_Value v);
static Scheduler_Status Promise_Reject(Promise_Promise p, Promise_Error e);
static Scheduler_Status Promise_GetFate(Promise_Future f, Promise_Fate *fate);
static Scheduler_Status Promise_GetResultIfSettled(Promise_Future f, int *settled, Promise_Result *res);
static Scheduler_Status Promise_Map(Scheduler_Scheduler s, Promise_Future f, Promise_ThenFn fn, void * user, Promise_Future *out);
static Scheduler_Status Promise_OnReject(Scheduler_Scheduler s, Promise_Future f, Promise_CatchFn fn, void * user, Promise_Future *out);
static Scheduler_Status Promise_OnSettle(Scheduler_Scheduler s, Promise_Future f, Promise_VoidFn fn, void * user, Promise_Future *out);
static Scheduler_Status Promise_All(Scheduler_Scheduler s, Promise_Future *fs, uint32_t fs_high, Promise_Future *out);
static Scheduler_Status Promise_Race(Scheduler_Scheduler s, Promise_Future *fs, uint32_t fs_high, Promise_Future *out);
static void Promise_InitCtPool(void);
static int Promise_AllocCancel(Promise_CancelPtr *p);
static Scheduler_Status Promise_CancelTokenCreate(Scheduler_Scheduler s, Promise_CancelToken *ct);
static void Promise_Cancel(Promise_CancelToken ct);
static int Promise_IsCancelled(Promise_CancelToken ct);
static void Promise_OnCancel(Promise_CancelToken ct, Promise_VoidFn fn, void * ctx);
static void Promise_CancellableThen(Promise_Result inRes, void * user, Promise_Result *outRes);
static Scheduler_Status Promise_MapCancellable(Scheduler_Scheduler s, Promise_Future f, Promise_ThenFn fn, void * user, Promise_CancelToken ct, Promise_Future *out);
static void Promise_MakeValue(int32_t tag, void * ptr, Promise_Value *v);
static void Promise_MakeError(int32_t code, void * ptr, Promise_Error *e);
static void Promise_Ok(Promise_Value v, Promise_Result *r);
static void Promise_Fail(Promise_Error e, Promise_Result *r);

Promise_SharedRec Promise_shPool[255 + 1];
uint32_t Promise_shFree[255 + 1];
int32_t Promise_shTop;
Promise_ContRec Promise_cnPool[511 + 1];
uint32_t Promise_cnFree[511 + 1];
int32_t Promise_cnTop;
int Promise_poolsReady;
Scheduler_TaskProc Promise_execContProc;
Promise_CancelRec Promise_ctPool[63 + 1];
uint32_t Promise_ctFree[63 + 1];
int32_t Promise_ctTop;
int Promise_ctReady;
Promise_CancMapRec Promise_cancMaps[63 + 1];
int32_t Promise_cancMapTop;
static void Promise_InitPools(void) {
    uint32_t i;
#line 79 "/Users/mattfitz/.mx/lib/m2futures/src/Promise.mod"
    for (i = 0; i <= (Promise_POOL_SH - 1); i += 1) {
#line 80
        Promise_shPool[i].poolIdx = i;
#line 81
        Promise_shPool[i].fate = Promise_Fate_Pending;
#line 82
        Promise_shPool[i].contHead = NULL;
#line 83
        Promise_shPool[i].contTail = NULL;
#line 84
        Promise_shFree[i] = i;
#line 85
    }
#line 86
    Promise_shTop = (Promise_POOL_SH - 1);
#line 88
    for (i = 0; i <= (Promise_POOL_CN - 1); i += 1) {
#line 89
        Promise_cnPool[i].poolIdx = i;
#line 90
        Promise_cnPool[i].next = NULL;
#line 91
        Promise_cnFree[i] = i;
#line 92
    }
#line 93
    Promise_cnTop = (Promise_POOL_CN - 1);
#line 95
    Promise_poolsReady = 1;
}

static int Promise_AllocShared(Promise_SharedPtr *p) {
    uint32_t idx;
#line 101
    if ((Promise_shTop < 0)) {
        return 0;
    }
#line 102
    idx = Promise_shFree[Promise_shTop];
#line 103
    Promise_shTop = (Promise_shTop - 1);
#line 104
    (*p) = ((void *)&(Promise_shPool[idx]));
#line 105
    return 1;
}

static void Promise_FreeShared(Promise_SharedPtr p) {
#line 110
    Promise_shTop = (Promise_shTop + 1);
#line 111
    Promise_shFree[Promise_shTop] = p->poolIdx;
}

static int Promise_AllocCont(Promise_ContPtr *c) {
    uint32_t idx;
#line 117
    if ((Promise_cnTop < 0)) {
        return 0;
    }
#line 118
    idx = Promise_cnFree[Promise_cnTop];
#line 119
    Promise_cnTop = (Promise_cnTop - 1);
#line 120
    (*c) = ((void *)&(Promise_cnPool[idx]));
#line 121
    return 1;
}

static void Promise_FreeCont(Promise_ContPtr c) {
#line 126
    Promise_cnTop = (Promise_cnTop + 1);
#line 127
    Promise_cnFree[Promise_cnTop] = c->poolIdx;
}

static void Promise_AppendCont(Promise_SharedPtr sh, Promise_ContPtr c) {
#line 136
    c->next = NULL;
#line 137
    if ((sh->contTail == NULL)) {
#line 138
        sh->contHead = c;
#line 139
        sh->contTail = c;
    } else {
#line 141
        sh->contTail->next = c;
#line 142
        sh->contTail = c;
    }
}

static void Promise_DrainConts(Promise_SharedPtr sh) {
    Promise_ContPtr c;
    Scheduler_Status st;
#line 151
    c = sh->contHead;
#line 152
    while ((c != NULL)) {
#line 153
        st = Scheduler_SchedulerEnqueue(sh->sched, Promise_execContProc, c);
#line 154
        c = c->next;
    }
#line 156
    sh->contHead = NULL;
#line 157
    sh->contTail = NULL;
}

static void Promise_SettleWith(Promise_SharedPtr sh, Promise_Result *res) {
#line 162
    if ((sh->fate != Promise_Fate_Pending)) {
        return;
    }
#line 163
    if ((*res).isOk) {
#line 164
        sh->fate = Promise_Fate_Fulfilled;
    } else {
#line 166
        sh->fate = Promise_Fate_Rejected;
    }
#line 168
    sh->res = (*res);
#line 169
    Promise_DrainConts(sh);
}

static void Promise_HandleAll(Promise_ContPtr c) {
    Promise_AllStatePtr asp;
    Promise_Result inRes, outRes;
    Promise_Value v;
    Promise_SharedPtr sh;
#line 179
    asp = c->combSt;
#line 180
    sh = c->inSh;
#line 181
    inRes = sh->res;
#line 183
    if (asp->failed) {
        return;
    }
#line 185
    if ((!inRes.isOk)) {
#line 186
        asp->failed = 1;
#line 187
        Promise_SettleWith(asp->outSh, &inRes);
    } else {
#line 189
        asp->results[c->idx] = inRes;
#line 190
        asp->done = (asp->done + 1);
#line 191
        if ((asp->done >= asp->total)) {
#line 192
            v.tag = asp->total;
#line 193
            v.ptr = ((void *)&(asp->results));
#line 194
            outRes.isOk = 1;
#line 195
            outRes.v = v;
#line 196
            Promise_SettleWith(asp->outSh, &outRes);
        }
    }
}

static void Promise_HandleRace(Promise_ContPtr c) {
    Promise_RaceStatePtr rsp;
    Promise_SharedPtr sh;
    Promise_Result inRes;
#line 207
    rsp = c->combSt;
#line 208
    if (rsp->settled) {
        return;
    }
#line 209
    rsp->settled = 1;
#line 210
    sh = c->inSh;
#line 211
    inRes = sh->res;
#line 212
    Promise_SettleWith(rsp->outSh, &inRes);
}

static void Promise_ExecuteCont(void * data) {
    Promise_ContPtr c;
    Promise_SharedPtr sh;
    Promise_Result inRes, outRes;
    Promise_ThenFn tf;
    Promise_CatchFn cf;
    Promise_VoidFn vf;
#line 226
    c = data;
#line 227
    sh = c->inSh;
#line 228
    inRes = sh->res;
#line 230
    if ((c->kind == Promise_ContKind_CKThen)) {
#line 231
        tf = c->thenFn;
#line 232
        tf(inRes, c->user, &outRes);
#line 233
        Promise_SettleWith(c->outSh, &outRes);
    } else if ((c->kind == Promise_ContKind_CKCatch)) {
#line 236
        if (inRes.isOk) {
#line 237
            outRes = inRes;
        } else {
#line 239
            cf = c->catchFn;
#line 240
            cf(inRes.e, c->user, &outRes);
        }
#line 242
        Promise_SettleWith(c->outSh, &outRes);
    } else if ((c->kind == Promise_ContKind_CKFinally)) {
#line 245
        vf = c->voidFn;
#line 246
        vf(inRes, c->user);
#line 247
        Promise_SettleWith(c->outSh, &inRes);
    } else if ((c->kind == Promise_ContKind_CKAll)) {
#line 250
        Promise_HandleAll(c);
    } else if ((c->kind == Promise_ContKind_CKRace)) {
#line 253
        Promise_HandleRace(c);
    }
#line 256
    Promise_FreeCont(c);
}

static Scheduler_Status Promise_PromiseCreate(Scheduler_Scheduler s, Promise_Promise *p, Promise_Future *f) {
    Promise_SharedPtr sh;
#line 268
    if ((!Promise_poolsReady)) {
        Promise_InitPools();
    }
#line 269
    if ((s == NULL)) {
#line 270
        (*p) = NULL;
        (*f) = NULL;
#line 271
        return Scheduler_Status_Invalid;
    }
#line 273
    if ((!Promise_AllocShared(&sh))) {
#line 274
        (*p) = NULL;
        (*f) = NULL;
#line 275
        return Scheduler_Status_OutOfMemory;
    }
#line 277
    sh->sched = s;
#line 278
    sh->fate = Promise_Fate_Pending;
#line 279
    sh->res.isOk = 0;
#line 280
    sh->contHead = NULL;
#line 281
    sh->contTail = NULL;
#line 282
    (*p) = sh;
#line 283
    (*f) = sh;
#line 284
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_Resolve(Promise_Promise p, Promise_Value v) {
    Promise_SharedPtr sh;
    Promise_Result res;
#line 296
    if ((p == NULL)) {
        return Scheduler_Status_Invalid;
    }
#line 297
    sh = p;
#line 298
    if ((sh->fate != Promise_Fate_Pending)) {
        return Scheduler_Status_AlreadySettled;
    }
#line 299
    res.isOk = 1;
#line 300
    res.v = v;
#line 301
    sh->fate = Promise_Fate_Fulfilled;
#line 302
    sh->res = res;
#line 303
    Promise_DrainConts(sh);
#line 304
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_Reject(Promise_Promise p, Promise_Error e) {
    Promise_SharedPtr sh;
    Promise_Result res;
#line 312
    if ((p == NULL)) {
        return Scheduler_Status_Invalid;
    }
#line 313
    sh = p;
#line 314
    if ((sh->fate != Promise_Fate_Pending)) {
        return Scheduler_Status_AlreadySettled;
    }
#line 315
    res.isOk = 0;
#line 316
    res.e = e;
#line 317
    sh->fate = Promise_Fate_Rejected;
#line 318
    sh->res = res;
#line 319
    Promise_DrainConts(sh);
#line 320
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_GetFate(Promise_Future f, Promise_Fate *fate) {
    Promise_SharedPtr sh;
#line 330
    if ((f == NULL)) {
        return Scheduler_Status_Invalid;
    }
#line 331
    sh = f;
#line 332
    (*fate) = sh->fate;
#line 333
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_GetResultIfSettled(Promise_Future f, int *settled, Promise_Result *res) {
    Promise_SharedPtr sh;
#line 341
    if ((f == NULL)) {
#line 342
        (*settled) = 0;
#line 343
        return Scheduler_Status_Invalid;
    }
#line 345
    sh = f;
#line 346
    if ((sh->fate == Promise_Fate_Pending)) {
#line 347
        (*settled) = 0;
    } else {
#line 349
        (*settled) = 1;
#line 350
        (*res) = sh->res;
    }
#line 352
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_Map(Scheduler_Scheduler s, Promise_Future f, Promise_ThenFn fn, void * user, Promise_Future *out) {
    Promise_SharedPtr inSh, outSh;
    Promise_ContPtr c;
    Promise_Promise p;
    Scheduler_Status st;
#line 368
    if (((s == NULL) || (f == NULL))) {
#line 369
        (*out) = NULL;
#line 370
        return Scheduler_Status_Invalid;
    }
#line 372
    inSh = f;
#line 373
    st = Promise_PromiseCreate(s, &p, out);
#line 374
    if ((st != Scheduler_Status_OK)) {
        return st;
    }
#line 375
    outSh = (*out);
#line 377
    if ((!Promise_AllocCont(&c))) {
#line 378
        (*out) = NULL;
#line 379
        return Scheduler_Status_OutOfMemory;
    }
#line 381
    c->kind = Promise_ContKind_CKThen;
#line 382
    c->thenFn = fn;
#line 383
    c->user = user;
#line 384
    c->inSh = inSh;
#line 385
    c->outSh = outSh;
#line 386
    c->next = NULL;
#line 388
    if ((inSh->fate != Promise_Fate_Pending)) {
#line 389
        st = Scheduler_SchedulerEnqueue(s, Promise_ExecuteCont, c);
    } else {
#line 391
        Promise_AppendCont(inSh, c);
    }
#line 393
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_OnReject(Scheduler_Scheduler s, Promise_Future f, Promise_CatchFn fn, void * user, Promise_Future *out) {
    Promise_SharedPtr inSh, outSh;
    Promise_ContPtr c;
    Promise_Promise p;
    Scheduler_Status st;
#line 405
    if (((s == NULL) || (f == NULL))) {
#line 406
        (*out) = NULL;
#line 407
        return Scheduler_Status_Invalid;
    }
#line 409
    inSh = f;
#line 410
    st = Promise_PromiseCreate(s, &p, out);
#line 411
    if ((st != Scheduler_Status_OK)) {
        return st;
    }
#line 412
    outSh = (*out);
#line 414
    if ((!Promise_AllocCont(&c))) {
#line 415
        (*out) = NULL;
#line 416
        return Scheduler_Status_OutOfMemory;
    }
#line 418
    c->kind = Promise_ContKind_CKCatch;
#line 419
    c->catchFn = fn;
#line 420
    c->user = user;
#line 421
    c->inSh = inSh;
#line 422
    c->outSh = outSh;
#line 423
    c->next = NULL;
#line 425
    if ((inSh->fate != Promise_Fate_Pending)) {
#line 426
        st = Scheduler_SchedulerEnqueue(s, Promise_ExecuteCont, c);
    } else {
#line 428
        Promise_AppendCont(inSh, c);
    }
#line 430
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_OnSettle(Scheduler_Scheduler s, Promise_Future f, Promise_VoidFn fn, void * user, Promise_Future *out) {
    Promise_SharedPtr inSh, outSh;
    Promise_ContPtr c;
    Promise_Promise p;
    Scheduler_Status st;
#line 442
    if (((s == NULL) || (f == NULL))) {
#line 443
        (*out) = NULL;
#line 444
        return Scheduler_Status_Invalid;
    }
#line 446
    inSh = f;
#line 447
    st = Promise_PromiseCreate(s, &p, out);
#line 448
    if ((st != Scheduler_Status_OK)) {
        return st;
    }
#line 449
    outSh = (*out);
#line 451
    if ((!Promise_AllocCont(&c))) {
#line 452
        (*out) = NULL;
#line 453
        return Scheduler_Status_OutOfMemory;
    }
#line 455
    c->kind = Promise_ContKind_CKFinally;
#line 456
    c->voidFn = fn;
#line 457
    c->user = user;
#line 458
    c->inSh = inSh;
#line 459
    c->outSh = outSh;
#line 460
    c->next = NULL;
#line 462
    if ((inSh->fate != Promise_Fate_Pending)) {
#line 463
        st = Scheduler_SchedulerEnqueue(s, Promise_ExecuteCont, c);
    } else {
#line 465
        Promise_AppendCont(inSh, c);
    }
#line 467
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_All(Scheduler_Scheduler s, Promise_Future *fs, uint32_t fs_high, Promise_Future *out) {
    uint32_t n, i;
    Promise_SharedPtr inSh;
    Promise_Promise p;
    Promise_SharedPtr outSh;
    Scheduler_Status st;
    Promise_AllStatePtr asp;
    Promise_ContPtr c;
#line 485
    n = (fs_high + 1);
#line 486
    if ((((s == NULL) || (n == 0)) || (n > Promise_MAX_ALL_SIZE))) {
#line 487
        (*out) = NULL;
#line 488
        return Scheduler_Status_Invalid;
    }
#line 490
    st = Promise_PromiseCreate(s, &p, out);
#line 491
    if ((st != Scheduler_Status_OK)) {
        return st;
    }
#line 492
    outSh = (*out);
#line 494
    asp = GC_MALLOC(sizeof(*asp));
#line 495
    if ((asp == NULL)) {
#line 496
        (*out) = NULL;
#line 497
        return Scheduler_Status_OutOfMemory;
    }
#line 499
    asp->outSh = outSh;
#line 500
    asp->total = n;
#line 501
    asp->done = 0;
#line 502
    asp->failed = 0;
#line 504
    for (i = 0; i <= (n - 1); i += 1) {
#line 505
        if ((!Promise_AllocCont(&c))) {
#line 506
            (*out) = NULL;
#line 507
            return Scheduler_Status_OutOfMemory;
        }
#line 509
        inSh = fs[i];
#line 510
        c->kind = Promise_ContKind_CKAll;
#line 511
        c->inSh = inSh;
#line 512
        c->outSh = outSh;
#line 513
        c->combSt = asp;
#line 514
        c->idx = i;
#line 515
        c->next = NULL;
#line 516
        c->user = NULL;
#line 518
        if ((inSh->fate != Promise_Fate_Pending)) {
#line 519
            st = Scheduler_SchedulerEnqueue(s, Promise_ExecuteCont, c);
        } else {
#line 521
            Promise_AppendCont(inSh, c);
        }
    }
#line 524
    return Scheduler_Status_OK;
}

static Scheduler_Status Promise_Race(Scheduler_Scheduler s, Promise_Future *fs, uint32_t fs_high, Promise_Future *out) {
    uint32_t n, i;
    Promise_SharedPtr inSh;
    Promise_Promise p;
    Promise_SharedPtr outSh;
    Scheduler_Status st;
    Promise_RaceStatePtr rsp;
    Promise_ContPtr c;
#line 538
    n = (fs_high + 1);
#line 539
    if (((s == NULL) || (n == 0))) {
#line 540
        (*out) = NULL;
#line 541
        return Scheduler_Status_Invalid;
    }
#line 543
    st = Promise_PromiseCreate(s, &p, out);
#line 544
    if ((st != Scheduler_Status_OK)) {
        return st;
    }
#line 545
    outSh = (*out);
#line 547
    rsp = GC_MALLOC(sizeof(*rsp));
#line 548
    if ((rsp == NULL)) {
#line 549
        (*out) = NULL;
#line 550
        return Scheduler_Status_OutOfMemory;
    }
#line 552
    rsp->outSh = outSh;
#line 553
    rsp->settled = 0;
#line 555
    for (i = 0; i <= (n - 1); i += 1) {
#line 556
        if ((!Promise_AllocCont(&c))) {
#line 557
            (*out) = NULL;
#line 558
            return Scheduler_Status_OutOfMemory;
        }
#line 560
        inSh = fs[i];
#line 561
        c->kind = Promise_ContKind_CKRace;
#line 562
        c->inSh = inSh;
#line 563
        c->outSh = outSh;
#line 564
        c->combSt = rsp;
#line 565
        c->idx = i;
#line 566
        c->next = NULL;
#line 567
        c->user = NULL;
#line 569
        if ((inSh->fate != Promise_Fate_Pending)) {
#line 570
            st = Scheduler_SchedulerEnqueue(s, Promise_ExecuteCont, c);
        } else {
#line 572
            Promise_AppendCont(inSh, c);
        }
    }
#line 575
    return Scheduler_Status_OK;
}

static void Promise_InitCtPool(void) {
    uint32_t i;
#line 611
    for (i = 0; i <= (Promise_POOL_CT - 1); i += 1) {
#line 612
        Promise_ctPool[i].poolIdx = i;
#line 613
        Promise_ctPool[i].cancelled = 0;
#line 614
        Promise_ctPool[i].cbCount = 0;
#line 615
        Promise_ctFree[i] = i;
#line 616
    }
#line 617
    Promise_ctTop = (Promise_POOL_CT - 1);
#line 618
    Promise_ctReady = 1;
}

static int Promise_AllocCancel(Promise_CancelPtr *p) {
    uint32_t idx;
#line 624
    if ((Promise_ctTop < 0)) {
        return 0;
    }
#line 625
    idx = Promise_ctFree[Promise_ctTop];
#line 626
    Promise_ctTop = (Promise_ctTop - 1);
#line 627
    (*p) = ((void *)&(Promise_ctPool[idx]));
#line 628
    return 1;
}

static Scheduler_Status Promise_CancelTokenCreate(Scheduler_Scheduler s, Promise_CancelToken *ct) {
    Promise_CancelPtr cp;
#line 634
    if ((!Promise_ctReady)) {
        Promise_InitCtPool();
    }
#line 635
    if ((s == NULL)) {
#line 636
        (*ct) = NULL;
#line 637
        return Scheduler_Status_Invalid;
    }
#line 639
    if ((!Promise_AllocCancel(&cp))) {
#line 640
        (*ct) = NULL;
#line 641
        return Scheduler_Status_OutOfMemory;
    }
#line 643
    cp->cancelled = 0;
#line 644
    cp->sched = s;
#line 645
    cp->cbCount = 0;
#line 646
    (*ct) = cp;
#line 647
    return Scheduler_Status_OK;
}

static void Promise_Cancel(Promise_CancelToken ct) {
    Promise_CancelPtr cp;
    int32_t i;
    Promise_Result r;
#line 656
    if ((ct == NULL)) {
        return;
    }
#line 657
    cp = ct;
#line 658
    if (cp->cancelled) {
        return;
    }
#line 659
    cp->cancelled = 1;
#line 660
    r.isOk = 0;
#line 661
    r.e.code = (-1);
#line 662
    r.e.ptr = NULL;
#line 663
    for (i = 0; i <= (cp->cbCount - 1); i += 1) {
#line 664
        cp->cbs[i].fn(r, cp->cbs[i].ctx);
    }
#line 666
    cp->cbCount = 0;
}

static int Promise_IsCancelled(Promise_CancelToken ct) {
    Promise_CancelPtr cp;
#line 672
    if ((ct == NULL)) {
        return 0;
    }
#line 673
    cp = ct;
#line 674
    return cp->cancelled;
}

static void Promise_OnCancel(Promise_CancelToken ct, Promise_VoidFn fn, void * ctx) {
    Promise_CancelPtr cp;
    Promise_Result r;
#line 682
    if ((ct == NULL)) {
        return;
    }
#line 683
    cp = ct;
#line 684
    if (cp->cancelled) {
#line 685
        r.isOk = 0;
#line 686
        r.e.code = (-1);
#line 687
        r.e.ptr = NULL;
#line 688
        fn(r, ctx);
#line 689
        return;
    }
#line 691
    if ((cp->cbCount < Promise_MaxCancelCBs)) {
#line 692
        cp->cbs[cp->cbCount].fn = fn;
#line 693
        cp->cbs[cp->cbCount].ctx = ctx;
#line 694
        (cp->cbCount++);
    }
}

static void Promise_CancellableThen(Promise_Result inRes, void * user, Promise_Result *outRes) {
    Promise_CancMapPtr cm;
    Promise_CancelPtr cp;
    Promise_Error e;
#line 705
    cm = user;
#line 706
    cp = cm->ct;
#line 707
    if (cp->cancelled) {
#line 708
        (*outRes).isOk = 0;
#line 709
        (*outRes).e.code = (-1);
#line 710
        (*outRes).e.ptr = NULL;
#line 711
        return;
    }
#line 713
    cm->fn(inRes, cm->user, outRes);
}

static Scheduler_Status Promise_MapCancellable(Scheduler_Scheduler s, Promise_Future f, Promise_ThenFn fn, void * user, Promise_CancelToken ct, Promise_Future *out) {
    Promise_CancMapPtr cm;
#line 734
    if ((Promise_cancMapTop < 0)) {
#line 735
        (*out) = NULL;
#line 736
        return Scheduler_Status_OutOfMemory;
    }
#line 738
    cm = ((void *)&(Promise_cancMaps[Promise_cancMapTop]));
#line 739
    (Promise_cancMapTop--);
#line 740
    cm->fn = fn;
#line 741
    cm->user = user;
#line 742
    cm->ct = ct;
#line 743
    return Promise_Map(s, f, Promise_CancellableThen, cm, out);
}

static void Promise_MakeValue(int32_t tag, void * ptr, Promise_Value *v) {
#line 752
    (*v).tag = tag;
#line 753
    (*v).ptr = ptr;
}

static void Promise_MakeError(int32_t code, void * ptr, Promise_Error *e) {
#line 758
    (*e).code = code;
#line 759
    (*e).ptr = ptr;
}

static void Promise_Ok(Promise_Value v, Promise_Result *r) {
#line 764
    (*r).isOk = 1;
#line 765
    (*r).v = v;
}

static void Promise_Fail(Promise_Error e, Promise_Result *r) {
#line 770
    (*r).isOk = 0;
#line 771
    (*r).e = e;
}

static void Promise_init(void) {
#line 775
    Promise_poolsReady = 0;
#line 776
    Promise_ctReady = 0;
#line 777
    Promise_cancMapTop = 63;
#line 778
    Promise_execContProc = Promise_ExecuteCont;
}

/* Imported Module ByteBuf */

typedef struct ByteBuf_BytesView ByteBuf_BytesView;
typedef struct ByteBuf_Buf ByteBuf_Buf;
static const int32_t ByteBuf_MaxBufCap = 10485760;
struct ByteBuf_BytesView {
    void * base;
    uint32_t len;
};

struct ByteBuf_Buf {
    void * data;
    uint32_t len;
    uint32_t cap;
};

typedef char *ByteBuf_CharPtr;

static char ByteBuf_PeekChar(void * base, uint32_t idx);
static void ByteBuf_PokeChar(void * base, uint32_t idx, char ch);
static void ByteBuf_CopyBytes(void * src, void * dst, uint32_t srcOff, uint32_t dstOff, uint32_t n);
static void ByteBuf_Init(ByteBuf_Buf *b, uint32_t initialCap);
static void ByteBuf_Free(ByteBuf_Buf *b);
static void ByteBuf_Clear(ByteBuf_Buf *b);
static int ByteBuf_Reserve(ByteBuf_Buf *b, uint32_t extra);
static void ByteBuf_AppendByte(ByteBuf_Buf *b, uint32_t x);
static void ByteBuf_AppendChars(ByteBuf_Buf *b, char *a, uint32_t a_high, uint32_t n);
static void ByteBuf_AppendView(ByteBuf_Buf *b, ByteBuf_BytesView v);
static uint32_t ByteBuf_GetByte(ByteBuf_Buf *b, uint32_t idx);
static void ByteBuf_SetByte(ByteBuf_Buf *b, uint32_t idx, uint32_t val);
static ByteBuf_BytesView ByteBuf_AsView(ByteBuf_Buf *b);
static void ByteBuf_Truncate(ByteBuf_Buf *b, uint32_t newLen);
static void * ByteBuf_DataPtr(ByteBuf_Buf *b);
static uint32_t ByteBuf_ViewGetByte(ByteBuf_BytesView v, uint32_t idx);

static char ByteBuf_PeekChar(void * base, uint32_t idx) {
    ByteBuf_CharPtr p;
#line 14 "libs/m2bytes/src/ByteBuf.mod"
    p = ((ByteBuf_CharPtr)((((uint64_t)(base)) + ((uint64_t)(idx)))));
#line 15
    return (*p);
}

static void ByteBuf_PokeChar(void * base, uint32_t idx, char ch) {
    ByteBuf_CharPtr p;
#line 21
    p = ((ByteBuf_CharPtr)((((uint64_t)(base)) + ((uint64_t)(idx)))));
#line 22
    (*p) = ch;
}

static void ByteBuf_CopyBytes(void * src, void * dst, uint32_t srcOff, uint32_t dstOff, uint32_t n) {
    uint32_t i;
#line 29
    i = 0;
#line 30
    while ((i < n)) {
#line 31
        ByteBuf_PokeChar(dst, (dstOff + i), ByteBuf_PeekChar(src, (srcOff + i)));
#line 32
        (i++);
    }
}

static void ByteBuf_Init(ByteBuf_Buf *b, uint32_t initialCap) {
    uint32_t c;
#line 41
    c = initialCap;
#line 42
    if ((c > ByteBuf_MaxBufCap)) {
        c = ByteBuf_MaxBufCap;
    }
#line 43
    if ((c == 0)) {
        c = 64;
    }
#line 44
    m2_ALLOCATE(&(*b).data, c);
#line 45
    (*b).len = 0;
#line 46
    (*b).cap = c;
}

static void ByteBuf_Free(ByteBuf_Buf *b) {
#line 51
    if (((*b).data != NULL)) {
#line 52
        m2_DEALLOCATE(&(*b).data, (*b).cap);
#line 53
        (*b).data = NULL;
    }
#line 55
    (*b).len = 0;
#line 56
    (*b).cap = 0;
}

static void ByteBuf_Clear(ByteBuf_Buf *b) {
#line 61
    (*b).len = 0;
}

static int ByteBuf_Reserve(ByteBuf_Buf *b, uint32_t extra) {
    uint32_t needed, newCap;
    void * newData;
#line 71
    needed = ((*b).len + extra);
#line 72
    if ((needed <= (*b).cap)) {
        return 1;
    }
#line 73
    if ((needed > ByteBuf_MaxBufCap)) {
        return 0;
    }
#line 76
    newCap = ((*b).cap * 2);
#line 77
    if ((newCap < needed)) {
        newCap = needed;
    }
#line 78
    if ((newCap > ByteBuf_MaxBufCap)) {
        newCap = ByteBuf_MaxBufCap;
    }
#line 80
    m2_ALLOCATE(&newData, newCap);
#line 81
    if ((newData == NULL)) {
        return 0;
    }
#line 84
    if (((*b).len > 0)) {
#line 85
        ByteBuf_CopyBytes((*b).data, newData, 0, 0, (*b).len);
    }
#line 89
    m2_DEALLOCATE(&(*b).data, (*b).cap);
#line 91
    (*b).data = newData;
#line 92
    (*b).cap = newCap;
#line 93
    return 1;
}

static void ByteBuf_AppendByte(ByteBuf_Buf *b, uint32_t x) {
#line 100
    if (ByteBuf_Reserve(b, 1)) {
#line 101
        ByteBuf_PokeChar((*b).data, (*b).len, ((char)(m2_mod(x, 256))));
#line 102
        ((*b).len++);
    }
}

static void ByteBuf_AppendChars(ByteBuf_Buf *b, char *a, uint32_t a_high, uint32_t n) {
    uint32_t count, i;
#line 109
    count = n;
#line 110
    if ((count > (a_high + 1))) {
        count = (a_high + 1);
    }
#line 111
    if ((count == 0)) {
        return;
    }
#line 112
    if (ByteBuf_Reserve(b, count)) {
#line 113
        i = 0;
#line 114
        while ((i < count)) {
#line 115
            ByteBuf_PokeChar((*b).data, ((*b).len + i), a[i]);
#line 116
            (i++);
        }
#line 118
        (*b).len = ((*b).len + count);
    }
}

static void ByteBuf_AppendView(ByteBuf_Buf *b, ByteBuf_BytesView v) {
#line 124
    if ((v.len == 0)) {
        return;
    }
#line 125
    if (ByteBuf_Reserve(b, v.len)) {
#line 126
        ByteBuf_CopyBytes(v.base, (*b).data, 0, (*b).len, v.len);
#line 127
        (*b).len = ((*b).len + v.len);
    }
}

static uint32_t ByteBuf_GetByte(ByteBuf_Buf *b, uint32_t idx) {
#line 135
    if ((idx >= (*b).len)) {
        return 0;
    }
#line 136
    return (((uint32_t)((unsigned char)(ByteBuf_PeekChar((*b).data, idx)))) % 256);
}

static void ByteBuf_SetByte(ByteBuf_Buf *b, uint32_t idx, uint32_t val) {
#line 141
    if ((idx >= (*b).len)) {
        return;
    }
#line 142
    ByteBuf_PokeChar((*b).data, idx, ((char)(m2_mod(val, 256))));
}

static ByteBuf_BytesView ByteBuf_AsView(ByteBuf_Buf *b) {
    ByteBuf_BytesView v;
#line 148
    v.base = (*b).data;
#line 149
    v.len = (*b).len;
#line 150
    return v;
}

static void ByteBuf_Truncate(ByteBuf_Buf *b, uint32_t newLen) {
#line 155
    if ((newLen < (*b).len)) {
        (*b).len = newLen;
    }
}

static void * ByteBuf_DataPtr(ByteBuf_Buf *b) {
#line 160
    return (*b).data;
}

static uint32_t ByteBuf_ViewGetByte(ByteBuf_BytesView v, uint32_t idx) {
#line 167
    if ((idx >= v.len)) {
        return 0;
    }
#line 168
    return (((uint32_t)((unsigned char)(ByteBuf_PeekChar(v.base, idx)))) % 256);
}

/* Imported Module RpcFrame */

typedef struct RpcFrame_FrameReader RpcFrame_FrameReader;
static const int32_t RpcFrame_MaxFrame = 65531;
static const int32_t RpcFrame_TsOk = 0;
static const int32_t RpcFrame_TsWouldBlock = 1;
static const int32_t RpcFrame_TsClosed = 2;
static const int32_t RpcFrame_TsError = 3;
typedef uint32_t (*RpcFrame_ReadFn)(void *, void *, uint32_t, uint32_t *);

typedef uint32_t (*RpcFrame_WriteFn)(void *, void *, uint32_t, uint32_t *);

typedef enum { RpcFrame_FrameStatus_FrmOk, RpcFrame_FrameStatus_FrmNeedMore, RpcFrame_FrameStatus_FrmClosed, RpcFrame_FrameStatus_FrmTooLarge, RpcFrame_FrameStatus_FrmError } RpcFrame_FrameStatus;
#define m2_min_RpcFrame_FrameStatus 0
#define m2_max_RpcFrame_FrameStatus 4

struct RpcFrame_FrameReader {
    uint32_t state;
    char lenBuf[3 + 1];
    uint32_t lenPos;
    ByteBuf_Buf payloadBuf;
    uint32_t payloadLen;
    uint32_t payloadPos;
    uint32_t maxFrame;
    RpcFrame_ReadFn readFn;
    void * readCtx;
};

static const int32_t RpcFrame_StReadLen = 0;
static const int32_t RpcFrame_StReadPayload = 1;
static const int32_t RpcFrame_ChunkSize = 1024;
static uint32_t RpcFrame_DecodeBE32(char *a, uint32_t a_high);
static void RpcFrame_EncodeBE32(uint32_t val, char *a, uint32_t a_high);
static uint32_t RpcFrame_CallRead(RpcFrame_ReadFn fn, void * ctx, void * buf, uint32_t max, uint32_t *got);
static uint32_t RpcFrame_CallWrite(RpcFrame_WriteFn fn, void * ctx, void * buf, uint32_t len, uint32_t *sent);
static void RpcFrame_InitFrameReader(RpcFrame_FrameReader *fr, uint32_t maxFrame, RpcFrame_ReadFn fn, void * ctx);
static void RpcFrame_TryReadFrame(RpcFrame_FrameReader *fr, ByteBuf_BytesView *out, RpcFrame_FrameStatus *status);
static void RpcFrame_ResetFrameReader(RpcFrame_FrameReader *fr);
static void RpcFrame_FreeFrameReader(RpcFrame_FrameReader *fr);
static void RpcFrame_WriteFrame(RpcFrame_WriteFn fn, void * ctx, ByteBuf_BytesView payload, int *ok);

static uint32_t RpcFrame_DecodeBE32(char *a, uint32_t a_high) {
#line 17 "libs/m2rpc/src/RpcFrame.mod"
    return ((((((uint32_t)((unsigned char)(a[0]))) * 16777216) + (((uint32_t)((unsigned char)(a[1]))) * 65536)) + (((uint32_t)((unsigned char)(a[2]))) * 256)) + ((uint32_t)((unsigned char)(a[3]))));
}

static void RpcFrame_EncodeBE32(uint32_t val, char *a, uint32_t a_high) {
#line 25
    a[0] = ((char)(m2_mod(m2_div(val, 16777216), 256)));
#line 26
    a[1] = ((char)(m2_mod(m2_div(val, 65536), 256)));
#line 27
    a[2] = ((char)(m2_mod(m2_div(val, 256), 256)));
#line 28
    a[3] = ((char)(m2_mod(val, 256)));
}

static uint32_t RpcFrame_CallRead(RpcFrame_ReadFn fn, void * ctx, void * buf, uint32_t max, uint32_t *got) {
    RpcFrame_ReadFn doRead;
#line 42
    doRead = fn;
#line 43
    return doRead(ctx, buf, max, got);
}

static uint32_t RpcFrame_CallWrite(RpcFrame_WriteFn fn, void * ctx, void * buf, uint32_t len, uint32_t *sent) {
    RpcFrame_WriteFn doWrite;
#line 51
    doWrite = fn;
#line 52
    return doWrite(ctx, buf, len, sent);
}

static void RpcFrame_InitFrameReader(RpcFrame_FrameReader *fr, uint32_t maxFrame, RpcFrame_ReadFn fn, void * ctx) {
#line 61
    (*fr).state = RpcFrame_StReadLen;
#line 62
    (*fr).lenPos = 0;
#line 63
    (*fr).payloadLen = 0;
#line 64
    (*fr).payloadPos = 0;
#line 65
    (*fr).readFn = fn;
#line 66
    (*fr).readCtx = ctx;
#line 67
    if ((maxFrame > RpcFrame_MaxFrame)) {
#line 68
        (*fr).maxFrame = RpcFrame_MaxFrame;
    } else {
#line 70
        (*fr).maxFrame = maxFrame;
    }
#line 72
    ByteBuf_Init(&(*fr).payloadBuf, 256);
}

static void RpcFrame_TryReadFrame(RpcFrame_FrameReader *fr, ByteBuf_BytesView *out, RpcFrame_FrameStatus *status) {
    char tmp[1023 + 1];
    uint32_t want, got, ts, i;
    RpcFrame_ReadFn rfn;
    void * rctx;
#line 84
    (*out).base = NULL;
#line 85
    (*out).len = 0;
#line 87
    rfn = (*fr).readFn;
#line 88
    rctx = (*fr).readCtx;
#line 91
    if (((*fr).state == RpcFrame_StReadLen)) {
#line 92
        while (((*fr).lenPos < 4)) {
#line 93
            want = (4 - (*fr).lenPos);
#line 94
            ts = RpcFrame_CallRead(rfn, rctx, ((void *)&(tmp)), want, &got);
#line 95
            if ((ts == RpcFrame_TsClosed)) {
#line 96
                if (((*fr).lenPos == 0)) {
#line 97
                    (*status) = RpcFrame_FrameStatus_FrmClosed;
                } else {
#line 99
                    (*status) = RpcFrame_FrameStatus_FrmError;
                }
#line 101
                return;
            } else if ((ts == RpcFrame_TsWouldBlock)) {
#line 103
                (*status) = RpcFrame_FrameStatus_FrmNeedMore;
#line 104
                return;
            } else if ((ts == RpcFrame_TsError)) {
#line 106
                (*status) = RpcFrame_FrameStatus_FrmError;
#line 107
                return;
            }
#line 109
            if ((got == 0)) {
#line 110
                (*status) = RpcFrame_FrameStatus_FrmNeedMore;
#line 111
                return;
            }
#line 113
            i = 0;
#line 114
            while ((i < got)) {
#line 115
                (*fr).lenBuf[(*fr).lenPos] = tmp[i];
#line 116
                ((*fr).lenPos++);
#line 117
                (i++);
            }
        }
#line 121
        (*fr).payloadLen = RpcFrame_DecodeBE32((*fr).lenBuf, (sizeof((*fr).lenBuf) / sizeof((*fr).lenBuf[0])) - 1);
#line 122
        if (((*fr).payloadLen > (*fr).maxFrame)) {
#line 123
            (*status) = RpcFrame_FrameStatus_FrmTooLarge;
#line 124
            return;
        }
#line 127
        if (((*fr).payloadLen == 0)) {
#line 128
            ByteBuf_Clear(&(*fr).payloadBuf);
#line 129
            (*out) = ByteBuf_AsView(&(*fr).payloadBuf);
#line 130
            (*fr).lenPos = 0;
#line 131
            (*status) = RpcFrame_FrameStatus_FrmOk;
#line 132
            return;
        }
#line 135
        ByteBuf_Clear(&(*fr).payloadBuf);
#line 136
        if ((!ByteBuf_Reserve(&(*fr).payloadBuf, (*fr).payloadLen))) {
#line 137
            (*status) = RpcFrame_FrameStatus_FrmError;
#line 138
            return;
        }
#line 140
        (*fr).payloadPos = 0;
#line 141
        (*fr).state = RpcFrame_StReadPayload;
    }
#line 145
    while (((*fr).payloadPos < (*fr).payloadLen)) {
#line 146
        want = ((*fr).payloadLen - (*fr).payloadPos);
#line 147
        if ((want > RpcFrame_ChunkSize)) {
            want = RpcFrame_ChunkSize;
        }
#line 148
        ts = RpcFrame_CallRead(rfn, rctx, ((void *)&(tmp)), want, &got);
#line 149
        if ((ts == RpcFrame_TsClosed)) {
#line 150
            (*status) = RpcFrame_FrameStatus_FrmError;
#line 151
            return;
        } else if ((ts == RpcFrame_TsWouldBlock)) {
#line 153
            (*status) = RpcFrame_FrameStatus_FrmNeedMore;
#line 154
            return;
        } else if ((ts == RpcFrame_TsError)) {
#line 156
            (*status) = RpcFrame_FrameStatus_FrmError;
#line 157
            return;
        }
#line 159
        if ((got == 0)) {
#line 160
            (*status) = RpcFrame_FrameStatus_FrmNeedMore;
#line 161
            return;
        }
#line 163
        i = 0;
#line 164
        while ((i < got)) {
#line 165
            ByteBuf_AppendByte(&(*fr).payloadBuf, ((uint32_t)((unsigned char)(tmp[i]))));
#line 166
            (i++);
        }
#line 168
        (*fr).payloadPos = ((*fr).payloadPos + got);
    }
#line 171
    (*out) = ByteBuf_AsView(&(*fr).payloadBuf);
#line 172
    (*fr).state = RpcFrame_StReadLen;
#line 173
    (*fr).lenPos = 0;
#line 174
    (*status) = RpcFrame_FrameStatus_FrmOk;
}

static void RpcFrame_ResetFrameReader(RpcFrame_FrameReader *fr) {
#line 179
    (*fr).state = RpcFrame_StReadLen;
#line 180
    (*fr).lenPos = 0;
#line 181
    (*fr).payloadLen = 0;
#line 182
    (*fr).payloadPos = 0;
#line 183
    ByteBuf_Clear(&(*fr).payloadBuf);
}

static void RpcFrame_FreeFrameReader(RpcFrame_FrameReader *fr) {
#line 188
    ByteBuf_Free(&(*fr).payloadBuf);
}

static void RpcFrame_WriteFrame(RpcFrame_WriteFn fn, void * ctx, ByteBuf_BytesView payload, int *ok) {
    ByteBuf_Buf frameBuf;
    char hdr[3 + 1];
    ByteBuf_BytesView view;
    uint32_t pos, sent, ts;
#line 202
    (*ok) = 0;
#line 205
    ByteBuf_Init(&frameBuf, (payload.len + 4));
#line 206
    RpcFrame_EncodeBE32(payload.len, hdr, (sizeof(hdr) / sizeof(hdr[0])) - 1);
#line 207
    ByteBuf_AppendChars(&frameBuf, hdr, (sizeof(hdr) / sizeof(hdr[0])) - 1, 4);
#line 208
    if ((payload.len > 0)) {
#line 209
        ByteBuf_AppendView(&frameBuf, payload);
    }
#line 212
    view = ByteBuf_AsView(&frameBuf);
#line 213
    pos = 0;
#line 214
    while ((pos < view.len)) {
#line 215
        ts = RpcFrame_CallWrite(fn, ctx, ((void *)((((uint64_t)(view.base)) + ((uint64_t)(pos))))), (view.len - pos), &sent);
#line 216
        if (((ts == RpcFrame_TsError) || (ts == RpcFrame_TsClosed))) {
#line 217
            ByteBuf_Free(&frameBuf);
#line 218
            return;
        }
#line 220
        if (((ts != RpcFrame_TsWouldBlock) && (sent > 0))) {
#line 221
            pos = (pos + sent);
        }
    }
#line 225
    ByteBuf_Free(&frameBuf);
#line 226
    (*ok) = 1;
}

/* Imported Module Codec */

typedef struct Codec_Reader Codec_Reader;
typedef struct Codec_Writer Codec_Writer;
struct Codec_Reader {
    ByteBuf_BytesView v;
    uint32_t pos;
};

struct Codec_Writer {
    ByteBuf_Buf * buf;
};

static uint32_t Codec_ViewByte(ByteBuf_BytesView *v, uint32_t idx);
static void Codec_InitReader(Codec_Reader *r, ByteBuf_BytesView v);
static uint32_t Codec_Remaining(Codec_Reader *r);
static uint32_t Codec_ReadU8(Codec_Reader *r, int *ok);
static uint32_t Codec_ReadU16LE(Codec_Reader *r, int *ok);
static uint32_t Codec_ReadU16BE(Codec_Reader *r, int *ok);
static uint32_t Codec_ReadU32LE(Codec_Reader *r, int *ok);
static uint32_t Codec_ReadU32BE(Codec_Reader *r, int *ok);
static int32_t Codec_ReadI32LE(Codec_Reader *r, int *ok);
static int32_t Codec_ReadI32BE(Codec_Reader *r, int *ok);
static void Codec_Skip(Codec_Reader *r, uint32_t n, int *ok);
static void Codec_ReadSlice(Codec_Reader *r, uint32_t n, ByteBuf_BytesView *out, int *ok);
static void Codec_InitWriter(Codec_Writer *w, ByteBuf_Buf *b);
static void Codec_WriteU8(Codec_Writer *w, uint32_t val);
static void Codec_WriteU16LE(Codec_Writer *w, uint32_t val);
static void Codec_WriteU16BE(Codec_Writer *w, uint32_t val);
static void Codec_WriteU32LE(Codec_Writer *w, uint32_t val);
static void Codec_WriteU32BE(Codec_Writer *w, uint32_t val);
static void Codec_WriteI32LE(Codec_Writer *w, int32_t val);
static void Codec_WriteI32BE(Codec_Writer *w, int32_t val);
static void Codec_WriteChars(Codec_Writer *w, char *a, uint32_t a_high, uint32_t n);
static void Codec_WriteVarU32(Codec_Writer *w, uint32_t val);
static uint32_t Codec_ReadVarU32(Codec_Reader *r, int *ok);
static uint32_t Codec_ZigZagEncode(int32_t val);
static int32_t Codec_ZigZagDecode(uint32_t val);
static void Codec_WriteVarI32(Codec_Writer *w, int32_t val);
static int32_t Codec_ReadVarI32(Codec_Reader *r, int *ok);

static uint32_t Codec_ViewByte(ByteBuf_BytesView *v, uint32_t idx) {
#line 11 "libs/m2bytes/src/Codec.mod"
    return ByteBuf_ViewGetByte((*v), idx);
}

static void Codec_InitReader(Codec_Reader *r, ByteBuf_BytesView v) {
#line 18
    (*r).v = v;
#line 19
    (*r).pos = 0;
}

static uint32_t Codec_Remaining(Codec_Reader *r) {
#line 24
    if (((*r).pos >= (*r).v.len)) {
        return 0;
    }
#line 25
    return ((*r).v.len - (*r).pos);
}

static uint32_t Codec_ReadU8(Codec_Reader *r, int *ok) {
    uint32_t val;
#line 31
    if (((*r).pos >= (*r).v.len)) {
        (*ok) = 0;
        return 0;
    }
#line 32
    val = Codec_ViewByte(&(*r).v, (*r).pos);
#line 33
    ((*r).pos++);
#line 34
    (*ok) = 1;
#line 35
    return val;
}

static uint32_t Codec_ReadU16LE(Codec_Reader *r, int *ok) {
    uint32_t lo, hi;
#line 41
    if ((((*r).pos + 2) > (*r).v.len)) {
        (*ok) = 0;
        return 0;
    }
#line 42
    lo = Codec_ViewByte(&(*r).v, (*r).pos);
#line 43
    hi = Codec_ViewByte(&(*r).v, ((*r).pos + 1));
#line 44
    (*r).pos = ((*r).pos + 2);
#line 45
    (*ok) = 1;
#line 46
    return (lo + (hi * 256));
}

static uint32_t Codec_ReadU16BE(Codec_Reader *r, int *ok) {
    uint32_t lo, hi;
#line 52
    if ((((*r).pos + 2) > (*r).v.len)) {
        (*ok) = 0;
        return 0;
    }
#line 53
    hi = Codec_ViewByte(&(*r).v, (*r).pos);
#line 54
    lo = Codec_ViewByte(&(*r).v, ((*r).pos + 1));
#line 55
    (*r).pos = ((*r).pos + 2);
#line 56
    (*ok) = 1;
#line 57
    return (lo + (hi * 256));
}

static uint32_t Codec_ReadU32LE(Codec_Reader *r, int *ok) {
    uint32_t b0, b1, b2, b3;
#line 63
    if ((((*r).pos + 4) > (*r).v.len)) {
        (*ok) = 0;
        return 0;
    }
#line 64
    b0 = Codec_ViewByte(&(*r).v, (*r).pos);
#line 65
    b1 = Codec_ViewByte(&(*r).v, ((*r).pos + 1));
#line 66
    b2 = Codec_ViewByte(&(*r).v, ((*r).pos + 2));
#line 67
    b3 = Codec_ViewByte(&(*r).v, ((*r).pos + 3));
#line 68
    (*r).pos = ((*r).pos + 4);
#line 69
    (*ok) = 1;
#line 70
    return (((b0 + (b1 * 256)) + (b2 * 65536)) + (b3 * 16777216));
}

static uint32_t Codec_ReadU32BE(Codec_Reader *r, int *ok) {
    uint32_t b0, b1, b2, b3;
#line 76
    if ((((*r).pos + 4) > (*r).v.len)) {
        (*ok) = 0;
        return 0;
    }
#line 77
    b3 = Codec_ViewByte(&(*r).v, (*r).pos);
#line 78
    b2 = Codec_ViewByte(&(*r).v, ((*r).pos + 1));
#line 79
    b1 = Codec_ViewByte(&(*r).v, ((*r).pos + 2));
#line 80
    b0 = Codec_ViewByte(&(*r).v, ((*r).pos + 3));
#line 81
    (*r).pos = ((*r).pos + 4);
#line 82
    (*ok) = 1;
#line 83
    return (((b0 + (b1 * 256)) + (b2 * 65536)) + (b3 * 16777216));
}

static int32_t Codec_ReadI32LE(Codec_Reader *r, int *ok) {
    uint32_t u;
#line 89
    u = Codec_ReadU32LE(r, ok);
#line 90
    if ((!(*ok))) {
        return 0;
    }
#line 91
    return ((int32_t)(u));
}

static int32_t Codec_ReadI32BE(Codec_Reader *r, int *ok) {
    uint32_t u;
#line 97
    u = Codec_ReadU32BE(r, ok);
#line 98
    if ((!(*ok))) {
        return 0;
    }
#line 99
    return ((int32_t)(u));
}

static void Codec_Skip(Codec_Reader *r, uint32_t n, int *ok) {
#line 104
    if ((((*r).pos + n) > (*r).v.len)) {
        (*ok) = 0;
        return;
    }
#line 105
    (*r).pos = ((*r).pos + n);
#line 106
    (*ok) = 1;
}

static void Codec_ReadSlice(Codec_Reader *r, uint32_t n, ByteBuf_BytesView *out, int *ok) {
#line 112
    if ((((*r).pos + n) > (*r).v.len)) {
#line 113
        (*ok) = 0;
#line 114
        (*out).base = NULL;
#line 115
        (*out).len = 0;
#line 116
        return;
    }
#line 118
    (*out).base = ((void *)((((uint64_t)((*r).v.base)) + ((uint64_t)((*r).pos)))));
#line 119
    (*out).len = n;
#line 120
    (*r).pos = ((*r).pos + n);
#line 121
    (*ok) = 1;
}

static void Codec_InitWriter(Codec_Writer *w, ByteBuf_Buf *b) {
#line 128
    (*w).buf = ((void *)&((*b)));
}

static void Codec_WriteU8(Codec_Writer *w, uint32_t val) {
#line 133
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(val, 256));
}

static void Codec_WriteU16LE(Codec_Writer *w, uint32_t val) {
#line 138
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(val, 256));
#line 139
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 256), 256));
}

static void Codec_WriteU16BE(Codec_Writer *w, uint32_t val) {
#line 144
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 256), 256));
#line 145
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(val, 256));
}

static void Codec_WriteU32LE(Codec_Writer *w, uint32_t val) {
#line 150
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(val, 256));
#line 151
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 256), 256));
#line 152
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 65536), 256));
#line 153
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 16777216), 256));
}

static void Codec_WriteU32BE(Codec_Writer *w, uint32_t val) {
#line 158
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 16777216), 256));
#line 159
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 65536), 256));
#line 160
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(m2_div(val, 256), 256));
#line 161
    ByteBuf_AppendByte(&(*(*w).buf), m2_mod(val, 256));
}

static void Codec_WriteI32LE(Codec_Writer *w, int32_t val) {
#line 166
    Codec_WriteU32LE(w, ((uint32_t)(val)));
}

static void Codec_WriteI32BE(Codec_Writer *w, int32_t val) {
#line 171
    Codec_WriteU32BE(w, ((uint32_t)(val)));
}

static void Codec_WriteChars(Codec_Writer *w, char *a, uint32_t a_high, uint32_t n) {
    uint32_t count, i;
#line 177
    count = n;
#line 178
    if ((count > (a_high + 1))) {
        count = (a_high + 1);
    }
#line 179
    i = 0;
#line 180
    while ((i < count)) {
#line 181
        ByteBuf_AppendByte(&(*(*w).buf), (((uint32_t)((unsigned char)(a[i]))) % 256));
#line 182
        (i++);
    }
}

static void Codec_WriteVarU32(Codec_Writer *w, uint32_t val) {
    uint32_t v;
#line 191
    v = val;
#line 192
    while ((v >= 128)) {
#line 193
        ByteBuf_AppendByte(&(*(*w).buf), ((v % 128) + 128));
#line 194
        v = (v / 128);
    }
#line 196
    ByteBuf_AppendByte(&(*(*w).buf), v);
}

static uint32_t Codec_ReadVarU32(Codec_Reader *r, int *ok) {
    uint32_t result, shift, b;
    uint32_t count;
    uint32_t savePos;
#line 205
    savePos = (*r).pos;
#line 206
    result = 0;
#line 207
    shift = 1;
#line 208
    count = 0;
#line 209
    for (;;) {
#line 210
        if (((*r).pos >= (*r).v.len)) {
#line 211
            (*r).pos = savePos;
#line 212
            (*ok) = 0;
#line 213
            return 0;
        }
#line 215
        b = Codec_ViewByte(&(*r).v, (*r).pos);
#line 216
        ((*r).pos++);
#line 217
        (count++);
#line 218
        result = (result + ((b % 128) * shift));
#line 219
        if ((b < 128)) {
#line 220
            (*ok) = 1;
#line 221
            return result;
        }
#line 223
        if ((count >= 5)) {
#line 225
            (*r).pos = savePos;
#line 226
            (*ok) = 0;
#line 227
            return 0;
        }
#line 229
        shift = (shift * 128);
    }
}

static uint32_t Codec_ZigZagEncode(int32_t val) {
#line 237
    if ((val >= 0)) {
#line 238
        return (((uint32_t)(val)) * 2);
    } else {
#line 240
        return ((((uint32_t)((-(val + 1)))) * 2) + 1);
    }
}

static int32_t Codec_ZigZagDecode(uint32_t val) {
#line 246
    if ((m2_mod(val, 2) == 0)) {
#line 247
        return ((int32_t)(m2_div(val, 2)));
    } else {
#line 249
        return ((-((int32_t)(m2_div(val, 2)))) - 1);
    }
}

static void Codec_WriteVarI32(Codec_Writer *w, int32_t val) {
#line 255
    Codec_WriteVarU32(w, Codec_ZigZagEncode(val));
}

static int32_t Codec_ReadVarI32(Codec_Reader *r, int *ok) {
    uint32_t u;
#line 261
    u = Codec_ReadVarU32(r, ok);
#line 262
    if ((!(*ok))) {
        return 0;
    }
#line 263
    return Codec_ZigZagDecode(u);
}

/* Imported Module RpcCodec */

static const int32_t RpcCodec_Version = 1;
static const int32_t RpcCodec_MsgRequest = 0;
static const int32_t RpcCodec_MsgResponse = 1;
static const int32_t RpcCodec_MsgError = 2;
static void RpcCodec_WriteHeader(Codec_Writer *w, uint32_t msgType, uint32_t requestId);
static void RpcCodec_EncodeRequest(ByteBuf_Buf *buf, uint32_t requestId, char *method, uint32_t method_high, uint32_t methodLen, ByteBuf_BytesView body);
static void RpcCodec_EncodeResponse(ByteBuf_Buf *buf, uint32_t requestId, ByteBuf_BytesView body);
static void RpcCodec_EncodeError(ByteBuf_Buf *buf, uint32_t requestId, uint32_t errCode, char *errMsg, uint32_t errMsg_high, uint32_t errMsgLen, ByteBuf_BytesView body);
static void RpcCodec_DecodeHeader(ByteBuf_BytesView payload, uint32_t *version, uint32_t *msgType, uint32_t *requestId, int *ok);
static void RpcCodec_DecodeRequest(ByteBuf_BytesView payload, uint32_t *requestId, ByteBuf_BytesView *method, ByteBuf_BytesView *body, int *ok);
static void RpcCodec_DecodeResponse(ByteBuf_BytesView payload, uint32_t *requestId, ByteBuf_BytesView *body, int *ok);
static void RpcCodec_DecodeError(ByteBuf_BytesView payload, uint32_t *requestId, uint32_t *errCode, ByteBuf_BytesView *errMsg, ByteBuf_BytesView *body, int *ok);

static void RpcCodec_WriteHeader(Codec_Writer *w, uint32_t msgType, uint32_t requestId) {
#line 12 "libs/m2rpc/src/RpcCodec.mod"
    Codec_WriteU8(w, RpcCodec_Version);
#line 13
    Codec_WriteU8(w, msgType);
#line 14
    Codec_WriteU32BE(w, requestId);
}

static void RpcCodec_EncodeRequest(ByteBuf_Buf *buf, uint32_t requestId, char *method, uint32_t method_high, uint32_t methodLen, ByteBuf_BytesView body) {
    Codec_Writer w;
    uint32_t ml;
#line 24
    ByteBuf_Clear(buf);
#line 25
    Codec_InitWriter(&w, buf);
#line 26
    RpcCodec_WriteHeader(&w, RpcCodec_MsgRequest, requestId);
#line 27
    ml = methodLen;
#line 28
    if ((ml > (method_high + 1))) {
        ml = (method_high + 1);
    }
#line 29
    Codec_WriteU16BE(&w, ml);
#line 30
    Codec_WriteChars(&w, method, method_high, ml);
#line 31
    Codec_WriteU32BE(&w, body.len);
#line 32
    if ((body.len > 0)) {
#line 33
        ByteBuf_AppendView(buf, body);
    }
}

static void RpcCodec_EncodeResponse(ByteBuf_Buf *buf, uint32_t requestId, ByteBuf_BytesView body) {
    Codec_Writer w;
#line 42
    ByteBuf_Clear(buf);
#line 43
    Codec_InitWriter(&w, buf);
#line 44
    RpcCodec_WriteHeader(&w, RpcCodec_MsgResponse, requestId);
#line 45
    Codec_WriteU32BE(&w, body.len);
#line 46
    if ((body.len > 0)) {
#line 47
        ByteBuf_AppendView(buf, body);
    }
}

static void RpcCodec_EncodeError(ByteBuf_Buf *buf, uint32_t requestId, uint32_t errCode, char *errMsg, uint32_t errMsg_high, uint32_t errMsgLen, ByteBuf_BytesView body) {
    Codec_Writer w;
    uint32_t ml;
#line 59
    ByteBuf_Clear(buf);
#line 60
    Codec_InitWriter(&w, buf);
#line 61
    RpcCodec_WriteHeader(&w, RpcCodec_MsgError, requestId);
#line 62
    Codec_WriteU16BE(&w, errCode);
#line 63
    ml = errMsgLen;
#line 64
    if ((ml > (errMsg_high + 1))) {
        ml = (errMsg_high + 1);
    }
#line 65
    Codec_WriteU16BE(&w, ml);
#line 66
    Codec_WriteChars(&w, errMsg, errMsg_high, ml);
#line 67
    Codec_WriteU32BE(&w, body.len);
#line 68
    if ((body.len > 0)) {
#line 69
        ByteBuf_AppendView(buf, body);
    }
}

static void RpcCodec_DecodeHeader(ByteBuf_BytesView payload, uint32_t *version, uint32_t *msgType, uint32_t *requestId, int *ok) {
    Codec_Reader r;
#line 82
    (*ok) = 0;
#line 83
    Codec_InitReader(&r, payload);
#line 84
    (*version) = Codec_ReadU8(&r, ok);
#line 85
    if ((!(*ok))) {
        return;
    }
#line 86
    (*msgType) = Codec_ReadU8(&r, ok);
#line 87
    if ((!(*ok))) {
        return;
    }
#line 88
    (*requestId) = Codec_ReadU32BE(&r, ok);
}

static void RpcCodec_DecodeRequest(ByteBuf_BytesView payload, uint32_t *requestId, ByteBuf_BytesView *method, ByteBuf_BytesView *body, int *ok) {
    Codec_Reader r;
    uint32_t ver, mt, ml, bl;
#line 100
    (*ok) = 0;
#line 101
    (*method).base = NULL;
    (*method).len = 0;
#line 102
    (*body).base = NULL;
    (*body).len = 0;
#line 103
    Codec_InitReader(&r, payload);
#line 106
    ver = Codec_ReadU8(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 107
    if ((ver != RpcCodec_Version)) {
        (*ok) = 0;
        return;
    }
#line 108
    mt = Codec_ReadU8(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 109
    if ((mt != RpcCodec_MsgRequest)) {
        (*ok) = 0;
        return;
    }
#line 110
    (*requestId) = Codec_ReadU32BE(&r, ok);
#line 111
    if ((!(*ok))) {
        return;
    }
#line 114
    ml = Codec_ReadU16BE(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 115
    Codec_ReadSlice(&r, ml, method, ok);
#line 116
    if ((!(*ok))) {
        return;
    }
#line 119
    bl = Codec_ReadU32BE(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 120
    if ((bl == 0)) {
#line 121
        (*body).base = NULL;
#line 122
        (*body).len = 0;
#line 123
        (*ok) = 1;
    } else {
#line 125
        Codec_ReadSlice(&r, bl, body, ok);
    }
}

static void RpcCodec_DecodeResponse(ByteBuf_BytesView payload, uint32_t *requestId, ByteBuf_BytesView *body, int *ok) {
    Codec_Reader r;
    uint32_t ver, mt, bl;
#line 137
    (*ok) = 0;
#line 138
    (*body).base = NULL;
    (*body).len = 0;
#line 139
    Codec_InitReader(&r, payload);
#line 141
    ver = Codec_ReadU8(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 142
    if ((ver != RpcCodec_Version)) {
        (*ok) = 0;
        return;
    }
#line 143
    mt = Codec_ReadU8(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 144
    if ((mt != RpcCodec_MsgResponse)) {
        (*ok) = 0;
        return;
    }
#line 145
    (*requestId) = Codec_ReadU32BE(&r, ok);
#line 146
    if ((!(*ok))) {
        return;
    }
#line 148
    bl = Codec_ReadU32BE(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 149
    if ((bl == 0)) {
#line 150
        (*body).base = NULL;
#line 151
        (*body).len = 0;
#line 152
        (*ok) = 1;
    } else {
#line 154
        Codec_ReadSlice(&r, bl, body, ok);
    }
}

static void RpcCodec_DecodeError(ByteBuf_BytesView payload, uint32_t *requestId, uint32_t *errCode, ByteBuf_BytesView *errMsg, ByteBuf_BytesView *body, int *ok) {
    Codec_Reader r;
    uint32_t ver, mt, ml, bl;
#line 168
    (*ok) = 0;
#line 169
    (*errMsg).base = NULL;
    (*errMsg).len = 0;
#line 170
    (*body).base = NULL;
    (*body).len = 0;
#line 171
    Codec_InitReader(&r, payload);
#line 173
    ver = Codec_ReadU8(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 174
    if ((ver != RpcCodec_Version)) {
        (*ok) = 0;
        return;
    }
#line 175
    mt = Codec_ReadU8(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 176
    if ((mt != RpcCodec_MsgError)) {
        (*ok) = 0;
        return;
    }
#line 177
    (*requestId) = Codec_ReadU32BE(&r, ok);
#line 178
    if ((!(*ok))) {
        return;
    }
#line 180
    (*errCode) = Codec_ReadU16BE(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 181
    ml = Codec_ReadU16BE(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 182
    Codec_ReadSlice(&r, ml, errMsg, ok);
#line 183
    if ((!(*ok))) {
        return;
    }
#line 185
    bl = Codec_ReadU32BE(&r, ok);
    if ((!(*ok))) {
        return;
    }
#line 186
    if ((bl == 0)) {
#line 187
        (*body).base = NULL;
#line 188
        (*body).len = 0;
#line 189
        (*ok) = 1;
    } else {
#line 191
        Codec_ReadSlice(&r, bl, body, ok);
    }
}

/* Imported Module RpcErrors */

static const int32_t RpcErrors_Ok = 0;
static const int32_t RpcErrors_BadRequest = 1;
static const int32_t RpcErrors_UnknownMethod = 2;
static const int32_t RpcErrors_Timeout = 3;
static const int32_t RpcErrors_Internal = 4;
static const int32_t RpcErrors_TooLarge = 5;
static const int32_t RpcErrors_Closed = 6;
static void RpcErrors_ToString(uint32_t code, char *s, uint32_t s_high);

static void RpcErrors_ToString(uint32_t code, char *s, uint32_t s_high) {
#line 7 "libs/m2rpc/src/RpcErrors.mod"
    if ((code == RpcErrors_Ok)) {
#line 8
        m2_Strings_Assign("Ok", s, s_high);
    } else if ((code == RpcErrors_BadRequest)) {
#line 10
        m2_Strings_Assign("BadRequest", s, s_high);
    } else if ((code == RpcErrors_UnknownMethod)) {
#line 12
        m2_Strings_Assign("UnknownMethod", s, s_high);
    } else if ((code == RpcErrors_Timeout)) {
#line 14
        m2_Strings_Assign("Timeout", s, s_high);
    } else if ((code == RpcErrors_Internal)) {
#line 16
        m2_Strings_Assign("Internal", s, s_high);
    } else if ((code == RpcErrors_TooLarge)) {
#line 18
        m2_Strings_Assign("TooLarge", s, s_high);
    } else if ((code == RpcErrors_Closed)) {
#line 20
        m2_Strings_Assign("Closed", s, s_high);
    } else {
#line 22
        m2_Strings_Assign("Unknown", s, s_high);
    }
}

/* Imported Module Timers */

typedef struct Timers_TimerEntry Timers_TimerEntry;
typedef struct Timers_QueueRec Timers_QueueRec;
static const int32_t Timers_MaxTimers = 256;
typedef int32_t Timers_TimerId;

typedef enum { Timers_Status_OK, Timers_Status_Invalid, Timers_Status_PoolExhausted } Timers_Status;
#define m2_min_Timers_Status 0
#define m2_max_Timers_Status 2

typedef void * Timers_TimerQueue;

struct Timers_TimerEntry {
    int32_t deadline;
    Scheduler_TaskProc cb;
    void * user;
    Timers_TimerId id;
    int32_t interval;
    int active;
};

struct Timers_QueueRec {
    Scheduler_Scheduler sched;
    int32_t heap[255 + 1];
    int32_t heapSize;
    Timers_TimerEntry pool[255 + 1];
    int32_t nextId;
};

typedef Timers_QueueRec *Timers_QueuePtr;

static int Timers_TimeBefore(int32_t a, int32_t b);
static void Timers_HeapSwap(Timers_QueueRec *q, int32_t i, int32_t j);
static void Timers_SiftUp(Timers_QueueRec *q, int32_t pos);
static void Timers_SiftDown(Timers_QueueRec *q, int32_t pos);
static void Timers_HeapPush(Timers_QueueRec *q, int32_t poolIdx);
static int32_t Timers_HeapPop(Timers_QueueRec *q);
static int Timers_AllocSlot(Timers_QueueRec *q, int32_t *slot);
static Timers_Status Timers_Create(Scheduler_Scheduler sched, Timers_TimerQueue *out);
static Timers_Status Timers_Destroy(Timers_TimerQueue *q);
static Timers_Status Timers_SetTimeout(Timers_TimerQueue q, int32_t now, int32_t delayMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id);
static Timers_Status Timers_SetInterval(Timers_TimerQueue q, int32_t now, int32_t intervalMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id);
static Timers_Status Timers_Cancel(Timers_TimerQueue q, Timers_TimerId id);
static int32_t Timers_ActiveCount(Timers_TimerQueue q);
static int32_t Timers_NextDeadline(Timers_TimerQueue q, int32_t now);
static Timers_Status Timers_Tick(Timers_TimerQueue q, int32_t now);

static int Timers_TimeBefore(int32_t a, int32_t b) {
#line 35 "/Users/mattfitz/.mx/lib/m2evloop/src/Timers.mod"
    return ((a - b) < 0);
}

static void Timers_HeapSwap(Timers_QueueRec *q, int32_t i, int32_t j) {
    int32_t tmp;
#line 43
    tmp = (*q).heap[i];
#line 44
    (*q).heap[i] = (*q).heap[j];
#line 45
    (*q).heap[j] = tmp;
}

static void Timers_SiftUp(Timers_QueueRec *q, int32_t pos) {
    int32_t parent;
#line 51
    while ((pos > 0)) {
#line 52
        parent = m2_div((pos - 1), 2);
#line 53
        if (Timers_TimeBefore((*q).pool[(*q).heap[pos]].deadline, (*q).pool[(*q).heap[parent]].deadline)) {
#line 55
            Timers_HeapSwap(q, pos, parent);
#line 56
            pos = parent;
        } else {
#line 58
            return;
        }
    }
}

static void Timers_SiftDown(Timers_QueueRec *q, int32_t pos) {
    int32_t left, right, smallest;
#line 66
    for (;;) {
#line 67
        left = ((2 * pos) + 1);
#line 68
        right = ((2 * pos) + 2);
#line 69
        smallest = pos;
#line 71
        if (((left < (*q).heapSize) && Timers_TimeBefore((*q).pool[(*q).heap[left]].deadline, (*q).pool[(*q).heap[smallest]].deadline))) {
#line 74
            smallest = left;
        }
#line 76
        if (((right < (*q).heapSize) && Timers_TimeBefore((*q).pool[(*q).heap[right]].deadline, (*q).pool[(*q).heap[smallest]].deadline))) {
#line 79
            smallest = right;
        }
#line 82
        if ((smallest == pos)) {
            return;
        }
#line 83
        Timers_HeapSwap(q, pos, smallest);
#line 84
        pos = smallest;
    }
}

static void Timers_HeapPush(Timers_QueueRec *q, int32_t poolIdx) {
#line 90
    (*q).heap[(*q).heapSize] = poolIdx;
#line 91
    ((*q).heapSize++);
#line 92
    Timers_SiftUp(q, ((*q).heapSize - 1));
}

static int32_t Timers_HeapPop(Timers_QueueRec *q) {
    int32_t top;
#line 98
    top = (*q).heap[0];
#line 99
    ((*q).heapSize--);
#line 100
    if (((*q).heapSize > 0)) {
#line 101
        (*q).heap[0] = (*q).heap[(*q).heapSize];
#line 102
        Timers_SiftDown(q, 0);
    }
#line 104
    return top;
}

static int Timers_AllocSlot(Timers_QueueRec *q, int32_t *slot) {
    int32_t i;
#line 112
    for (i = 0; i <= (Timers_MaxTimers - 1); i += 1) {
#line 113
        if ((!(*q).pool[i].active)) {
#line 114
            (*slot) = i;
#line 115
            return 1;
        }
    }
#line 118
    return 0;
}

static Timers_Status Timers_Create(Scheduler_Scheduler sched, Timers_TimerQueue *out) {
    Timers_QueuePtr qp;
    int32_t i;
#line 127
    if ((sched == NULL)) {
#line 128
        (*out) = NULL;
#line 129
        return Timers_Status_Invalid;
    }
#line 131
    m2_ALLOCATE(&qp, ((uint32_t)sizeof(Timers_QueueRec)));
#line 132
    if ((qp == NULL)) {
#line 133
        (*out) = NULL;
#line 134
        return Timers_Status_PoolExhausted;
    }
#line 136
    qp->sched = sched;
#line 137
    qp->heapSize = 0;
#line 138
    qp->nextId = 1;
#line 139
    for (i = 0; i <= (Timers_MaxTimers - 1); i += 1) {
#line 140
        qp->pool[i].active = 0;
    }
#line 142
    (*out) = qp;
#line 143
    return Timers_Status_OK;
}

static Timers_Status Timers_Destroy(Timers_TimerQueue *q) {
    Timers_QueuePtr qp;
#line 149
    if (((*q) == NULL)) {
        return Timers_Status_Invalid;
    }
#line 150
    qp = (*q);
#line 151
    m2_DEALLOCATE(&qp, ((uint32_t)sizeof(Timers_QueueRec)));
#line 152
    (*q) = NULL;
#line 153
    return Timers_Status_OK;
}

static Timers_Status Timers_SetTimeout(Timers_TimerQueue q, int32_t now, int32_t delayMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id) {
    Timers_QueuePtr qp;
    int32_t slot;
#line 161
    if ((q == NULL)) {
        return Timers_Status_Invalid;
    }
#line 162
    qp = q;
#line 163
    if ((!Timers_AllocSlot(&(*qp), &slot))) {
#line 164
        return Timers_Status_PoolExhausted;
    }
#line 166
    qp->pool[slot].deadline = (now + delayMs);
#line 167
    qp->pool[slot].cb = cb;
#line 168
    qp->pool[slot].user = user;
#line 169
    qp->pool[slot].id = qp->nextId;
#line 170
    qp->pool[slot].interval = 0;
#line 171
    qp->pool[slot].active = 1;
#line 172
    (*id) = qp->nextId;
#line 173
    (qp->nextId++);
#line 174
    Timers_HeapPush(&(*qp), slot);
#line 175
    return Timers_Status_OK;
}

static Timers_Status Timers_SetInterval(Timers_TimerQueue q, int32_t now, int32_t intervalMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id) {
    Timers_QueuePtr qp;
    int32_t slot;
#line 183
    if ((q == NULL)) {
        return Timers_Status_Invalid;
    }
#line 184
    qp = q;
#line 185
    if ((!Timers_AllocSlot(&(*qp), &slot))) {
#line 186
        return Timers_Status_PoolExhausted;
    }
#line 188
    qp->pool[slot].deadline = (now + intervalMs);
#line 189
    qp->pool[slot].cb = cb;
#line 190
    qp->pool[slot].user = user;
#line 191
    qp->pool[slot].id = qp->nextId;
#line 192
    qp->pool[slot].interval = intervalMs;
#line 193
    qp->pool[slot].active = 1;
#line 194
    (*id) = qp->nextId;
#line 195
    (qp->nextId++);
#line 196
    Timers_HeapPush(&(*qp), slot);
#line 197
    return Timers_Status_OK;
}

static Timers_Status Timers_Cancel(Timers_TimerQueue q, Timers_TimerId id) {
    Timers_QueuePtr qp;
    int32_t i;
#line 203
    if ((q == NULL)) {
        return Timers_Status_Invalid;
    }
#line 204
    qp = q;
#line 206
    for (i = 0; i <= (Timers_MaxTimers - 1); i += 1) {
#line 207
        if ((qp->pool[i].active && (qp->pool[i].id == id))) {
#line 208
            qp->pool[i].active = 0;
#line 209
            return Timers_Status_OK;
        }
    }
#line 212
    return Timers_Status_OK;
}

static int32_t Timers_ActiveCount(Timers_TimerQueue q) {
    Timers_QueuePtr qp;
    int32_t i, count;
#line 218
    if ((q == NULL)) {
        return 0;
    }
#line 219
    qp = q;
#line 220
    count = 0;
#line 221
    for (i = 0; i <= (Timers_MaxTimers - 1); i += 1) {
#line 222
        if (qp->pool[i].active) {
            (count++);
        }
    }
#line 224
    return count;
}

static int32_t Timers_NextDeadline(Timers_TimerQueue q, int32_t now) {
    Timers_QueuePtr qp;
    int32_t diff;
#line 230
    if ((q == NULL)) {
        return (-1);
    }
#line 231
    qp = q;
#line 233
    while (((qp->heapSize > 0) && (!qp->pool[qp->heap[0]].active))) {
#line 235
        (qp->heapSize--);
#line 236
        if ((qp->heapSize > 0)) {
#line 237
            qp->heap[0] = qp->heap[qp->heapSize];
#line 238
            Timers_SiftDown(&(*qp), 0);
        }
    }
#line 241
    if ((qp->heapSize == 0)) {
        return (-1);
    }
#line 242
    diff = (qp->pool[qp->heap[0]].deadline - now);
#line 243
    if ((diff < 0)) {
        return 0;
    }
#line 244
    return diff;
}

static Timers_Status Timers_Tick(Timers_TimerQueue q, int32_t now) {
    Timers_QueuePtr qp;
    int32_t idx;
    Scheduler_Status dummy;
#line 253
    if ((q == NULL)) {
        return Timers_Status_Invalid;
    }
#line 254
    qp = q;
#line 255
    for (;;) {
#line 257
        while (((qp->heapSize > 0) && (!qp->pool[qp->heap[0]].active))) {
#line 259
            idx = Timers_HeapPop(&(*qp));
        }
#line 262
        if ((qp->heapSize == 0)) {
            break;
        }
#line 264
        idx = qp->heap[0];
#line 265
        if (Timers_TimeBefore(now, qp->pool[idx].deadline)) {
#line 266
            break;
        }
#line 270
        idx = Timers_HeapPop(&(*qp));
#line 271
        if (qp->pool[idx].active) {
#line 273
            dummy = Scheduler_SchedulerEnqueue(qp->sched, qp->pool[idx].cb, qp->pool[idx].user);
#line 277
            if ((qp->pool[idx].interval > 0)) {
#line 279
                qp->pool[idx].deadline = (qp->pool[idx].deadline + qp->pool[idx].interval);
#line 281
                Timers_HeapPush(&(*qp), idx);
            } else {
#line 284
                qp->pool[idx].active = 0;
            }
        }
    }
#line 288
    return Timers_Status_OK;
}

/* Imported Module Poller */

typedef struct Poller_PollEvent Poller_PollEvent;
static const int32_t Poller_EvRead = 1;
static const int32_t Poller_EvWrite = 2;
static const int32_t Poller_EvError = 4;
static const int32_t Poller_EvHup = 8;
static const int32_t Poller_MaxEvents = 64;
typedef int32_t Poller_Poller;

struct Poller_PollEvent {
    int32_t fd;
    int32_t events;
};

typedef Poller_PollEvent Poller_EventBuf[63 + 1];

typedef enum { Poller_Status_OK, Poller_Status_SysError, Poller_Status_Invalid } Poller_Status;
#define m2_min_Poller_Status 0
#define m2_max_Poller_Status 2

static Poller_Status Poller_Create(Poller_Poller *out);
static Poller_Status Poller_Destroy(Poller_Poller *p);
static Poller_Status Poller_Add(Poller_Poller p, int32_t fd, int32_t events);
static Poller_Status Poller_Modify(Poller_Poller p, int32_t fd, int32_t events);
static Poller_Status Poller_Remove(Poller_Poller p, int32_t fd);
static Poller_Status Poller_Wait(Poller_Poller p, int32_t timeoutMs, Poller_EventBuf *buf, int32_t *count);
static int32_t Poller_NowMs(void);

static Poller_Status Poller_Create(Poller_Poller *out) {
    int32_t h;
#line 11 "/Users/mattfitz/.mx/lib/m2evloop/src/Poller.mod"
    h = m2_poller_create();
#line 12
    if ((h < 0)) {
#line 13
        (*out) = (-1);
#line 14
        return Poller_Status_SysError;
    }
#line 16
    (*out) = h;
#line 17
    return Poller_Status_OK;
}

static Poller_Status Poller_Destroy(Poller_Poller *p) {
#line 22
    if (((*p) < 0)) {
        return Poller_Status_Invalid;
    }
#line 23
    m2_poller_destroy((*p));
#line 24
    (*p) = (-1);
#line 25
    return Poller_Status_OK;
}

static Poller_Status Poller_Add(Poller_Poller p, int32_t fd, int32_t events) {
#line 30
    if ((p < 0)) {
        return Poller_Status_Invalid;
    }
#line 31
    if ((m2_poller_add(p, fd, events) < 0)) {
#line 32
        return Poller_Status_SysError;
    }
#line 34
    return Poller_Status_OK;
}

static Poller_Status Poller_Modify(Poller_Poller p, int32_t fd, int32_t events) {
#line 39
    if ((p < 0)) {
        return Poller_Status_Invalid;
    }
#line 40
    if ((m2_poller_mod(p, fd, events) < 0)) {
#line 41
        return Poller_Status_SysError;
    }
#line 43
    return Poller_Status_OK;
}

static Poller_Status Poller_Remove(Poller_Poller p, int32_t fd) {
#line 48
    if ((p < 0)) {
        return Poller_Status_Invalid;
    }
#line 49
    if ((m2_poller_del(p, fd) < 0)) {
#line 50
        return Poller_Status_SysError;
    }
#line 52
    return Poller_Status_OK;
}

static Poller_Status Poller_Wait(Poller_Poller p, int32_t timeoutMs, Poller_EventBuf *buf, int32_t *count) {
    int32_t n;
#line 60
    if ((p < 0)) {
#line 61
        (*count) = 0;
#line 62
        return Poller_Status_Invalid;
    }
#line 64
    n = m2_poller_wait(p, timeoutMs, ((void *)&((*buf))), Poller_MaxEvents);
#line 65
    if ((n < 0)) {
#line 66
        (*count) = 0;
#line 67
        return Poller_Status_SysError;
    }
#line 69
    (*count) = n;
#line 70
    return Poller_Status_OK;
}

static int32_t Poller_NowMs(void) {
#line 75
    return m2_now_ms();
}

/* Imported Module EventLoop */

typedef struct EventLoop_WatcherEntry EventLoop_WatcherEntry;
typedef struct EventLoop_LoopRec EventLoop_LoopRec;
typedef void (*EventLoop_WatcherProc)(int32_t, int32_t, void *);

typedef void * EventLoop_Loop;

typedef enum { EventLoop_Status_OK, EventLoop_Status_Invalid, EventLoop_Status_SysError, EventLoop_Status_PoolExhausted } EventLoop_Status;
#define m2_min_EventLoop_Status 0
#define m2_max_EventLoop_Status 3

static const int32_t EventLoop_MaxWatchers = 512;
static const int32_t EventLoop_SchedCapacity = 1024;
struct EventLoop_WatcherEntry {
    int32_t fd;
    int32_t events;
    EventLoop_WatcherProc cb;
    void * user;
    int active;
};

struct EventLoop_LoopRec {
    int32_t poller;
    Timers_TimerQueue timers;
    Scheduler_Scheduler sched;
    EventLoop_WatcherEntry watchers[511 + 1];
    int32_t nWatchers;
    int running;
    int stopFlag;
};

typedef EventLoop_LoopRec *EventLoop_LoopPtr;

static int32_t EventLoop_FindWatcher(EventLoop_LoopPtr lp, int32_t fd);
static EventLoop_Status EventLoop_Create(EventLoop_Loop *out);
static EventLoop_Status EventLoop_Destroy(EventLoop_Loop *lp);
static EventLoop_Status EventLoop_SetTimeout(EventLoop_Loop lp, int32_t delayMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id);
static EventLoop_Status EventLoop_SetInterval(EventLoop_Loop lp, int32_t intervalMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id);
static EventLoop_Status EventLoop_CancelTimer(EventLoop_Loop lp, Timers_TimerId id);
static EventLoop_Status EventLoop_WatchFd(EventLoop_Loop lp, int32_t fd, int32_t events, EventLoop_WatcherProc cb, void * user);
static EventLoop_Status EventLoop_ModifyFd(EventLoop_Loop lp, int32_t fd, int32_t events);
static EventLoop_Status EventLoop_UnwatchFd(EventLoop_Loop lp, int32_t fd);
static EventLoop_Status EventLoop_Enqueue(EventLoop_Loop lp, Scheduler_TaskProc cb, void * user);
static Scheduler_Scheduler EventLoop_GetScheduler(EventLoop_Loop lp);
static int EventLoop_RunOnce(EventLoop_Loop lp);
static void EventLoop_Run(EventLoop_Loop lp);
static void EventLoop_Stop(EventLoop_Loop lp);

static int32_t EventLoop_FindWatcher(EventLoop_LoopPtr lp, int32_t fd) {
    int32_t i;
#line 46 "/Users/mattfitz/.mx/lib/m2evloop/src/EventLoop.mod"
    for (i = 0; i <= (EventLoop_MaxWatchers - 1); i += 1) {
#line 47
        if ((lp->watchers[i].active && (lp->watchers[i].fd == fd))) {
#line 48
            return i;
        }
    }
#line 51
    return (-1);
}

static EventLoop_Status EventLoop_Create(EventLoop_Loop *out) {
    EventLoop_LoopPtr lp;
    Poller_Status pst;
    Scheduler_Status sst;
    Timers_Status tst;
    int32_t i;
#line 64
    m2_ALLOCATE(&lp, ((uint32_t)sizeof(EventLoop_LoopRec)));
#line 65
    if ((lp == NULL)) {
#line 66
        (*out) = NULL;
#line 67
        return EventLoop_Status_PoolExhausted;
    }
#line 71
    pst = Poller_Create(&lp->poller);
#line 72
    if ((pst != Poller_Status_OK)) {
#line 73
        m2_DEALLOCATE(&lp, ((uint32_t)sizeof(EventLoop_LoopRec)));
#line 74
        (*out) = NULL;
#line 75
        return EventLoop_Status_SysError;
    }
#line 79
    sst = Scheduler_SchedulerCreate(EventLoop_SchedCapacity, &lp->sched);
#line 80
    if ((sst != Scheduler_Status_OK)) {
#line 81
        pst = Poller_Destroy(&lp->poller);
#line 82
        m2_DEALLOCATE(&lp, ((uint32_t)sizeof(EventLoop_LoopRec)));
#line 83
        (*out) = NULL;
#line 84
        return EventLoop_Status_PoolExhausted;
    }
#line 88
    tst = Timers_Create(lp->sched, &lp->timers);
#line 89
    if ((tst != Timers_Status_OK)) {
#line 90
        sst = Scheduler_SchedulerDestroy(&lp->sched);
#line 91
        pst = Poller_Destroy(&lp->poller);
#line 92
        m2_DEALLOCATE(&lp, ((uint32_t)sizeof(EventLoop_LoopRec)));
#line 93
        (*out) = NULL;
#line 94
        return EventLoop_Status_PoolExhausted;
    }
#line 97
    lp->nWatchers = 0;
#line 98
    lp->running = 0;
#line 99
    lp->stopFlag = 0;
#line 100
    for (i = 0; i <= (EventLoop_MaxWatchers - 1); i += 1) {
#line 101
        lp->watchers[i].active = 0;
    }
#line 104
    (*out) = lp;
#line 105
    return EventLoop_Status_OK;
}

static EventLoop_Status EventLoop_Destroy(EventLoop_Loop *lp) {
    EventLoop_LoopPtr p;
    Timers_Status tst;
    Scheduler_Status sst;
    Poller_Status pst;
#line 111
    if (((*lp) == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 112
    p = (*lp);
#line 113
    tst = Timers_Destroy(&p->timers);
#line 114
    sst = Scheduler_SchedulerDestroy(&p->sched);
#line 115
    pst = Poller_Destroy(&p->poller);
#line 116
    m2_DEALLOCATE(&p, ((uint32_t)sizeof(EventLoop_LoopRec)));
#line 117
    (*lp) = NULL;
#line 118
    return EventLoop_Status_OK;
}

static EventLoop_Status EventLoop_SetTimeout(EventLoop_Loop lp, int32_t delayMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id) {
    EventLoop_LoopPtr p;
    int32_t now;
    Timers_Status tst;
#line 128
    if ((lp == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 129
    p = lp;
#line 130
    now = Poller_NowMs();
#line 131
    tst = Timers_SetTimeout(p->timers, now, delayMs, cb, user, id);
#line 132
    if ((tst != Timers_Status_OK)) {
        return EventLoop_Status_PoolExhausted;
    }
#line 133
    return EventLoop_Status_OK;
}

static EventLoop_Status EventLoop_SetInterval(EventLoop_Loop lp, int32_t intervalMs, Scheduler_TaskProc cb, void * user, Timers_TimerId *id) {
    EventLoop_LoopPtr p;
    int32_t now;
    Timers_Status tst;
#line 141
    if ((lp == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 142
    p = lp;
#line 143
    now = Poller_NowMs();
#line 144
    tst = Timers_SetInterval(p->timers, now, intervalMs, cb, user, id);
#line 145
    if ((tst != Timers_Status_OK)) {
        return EventLoop_Status_PoolExhausted;
    }
#line 146
    return EventLoop_Status_OK;
}

static EventLoop_Status EventLoop_CancelTimer(EventLoop_Loop lp, Timers_TimerId id) {
    EventLoop_LoopPtr p;
    Timers_Status tst;
#line 152
    if ((lp == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 153
    p = lp;
#line 154
    tst = Timers_Cancel(p->timers, id);
#line 155
    return EventLoop_Status_OK;
}

static EventLoop_Status EventLoop_WatchFd(EventLoop_Loop lp, int32_t fd, int32_t events, EventLoop_WatcherProc cb, void * user) {
    EventLoop_LoopPtr p;
    int32_t i;
    Poller_Status pst;
#line 164
    if ((lp == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 165
    p = lp;
#line 166
    if ((p->nWatchers >= EventLoop_MaxWatchers)) {
        return EventLoop_Status_PoolExhausted;
    }
#line 168
    pst = Poller_Add(p->poller, fd, events);
#line 169
    if ((pst != Poller_Status_OK)) {
        return EventLoop_Status_SysError;
    }
#line 172
    for (i = 0; i <= (EventLoop_MaxWatchers - 1); i += 1) {
#line 173
        if ((!p->watchers[i].active)) {
#line 174
            p->watchers[i].fd = fd;
#line 175
            p->watchers[i].events = events;
#line 176
            p->watchers[i].cb = cb;
#line 177
            p->watchers[i].user = user;
#line 178
            p->watchers[i].active = 1;
#line 179
            (p->nWatchers++);
#line 180
            return EventLoop_Status_OK;
        }
    }
#line 183
    return EventLoop_Status_PoolExhausted;
}

static EventLoop_Status EventLoop_ModifyFd(EventLoop_Loop lp, int32_t fd, int32_t events) {
    EventLoop_LoopPtr p;
    int32_t idx;
    Poller_Status pst;
#line 189
    if ((lp == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 190
    p = lp;
#line 191
    idx = EventLoop_FindWatcher(p, fd);
#line 192
    if ((idx < 0)) {
        return EventLoop_Status_Invalid;
    }
#line 193
    pst = Poller_Modify(p->poller, fd, events);
#line 194
    if ((pst != Poller_Status_OK)) {
        return EventLoop_Status_SysError;
    }
#line 195
    p->watchers[idx].events = events;
#line 196
    return EventLoop_Status_OK;
}

static EventLoop_Status EventLoop_UnwatchFd(EventLoop_Loop lp, int32_t fd) {
    EventLoop_LoopPtr p;
    int32_t idx;
    Poller_Status pst;
#line 202
    if ((lp == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 203
    p = lp;
#line 204
    idx = EventLoop_FindWatcher(p, fd);
#line 205
    if ((idx < 0)) {
        return EventLoop_Status_Invalid;
    }
#line 206
    pst = Poller_Remove(p->poller, fd);
#line 207
    p->watchers[idx].active = 0;
#line 208
    (p->nWatchers--);
#line 209
    return EventLoop_Status_OK;
}

static EventLoop_Status EventLoop_Enqueue(EventLoop_Loop lp, Scheduler_TaskProc cb, void * user) {
    EventLoop_LoopPtr p;
    Scheduler_Status sst;
#line 217
    if ((lp == NULL)) {
        return EventLoop_Status_Invalid;
    }
#line 218
    p = lp;
#line 219
    sst = Scheduler_SchedulerEnqueue(p->sched, cb, user);
#line 220
    if ((sst != Scheduler_Status_OK)) {
        return EventLoop_Status_PoolExhausted;
    }
#line 221
    return EventLoop_Status_OK;
}

static Scheduler_Scheduler EventLoop_GetScheduler(EventLoop_Loop lp) {
    EventLoop_LoopPtr p;
#line 227
    if ((lp == NULL)) {
        return NULL;
    }
#line 228
    p = lp;
#line 229
    return p->sched;
}

static int EventLoop_RunOnce(EventLoop_Loop lp) {
    EventLoop_LoopPtr p;
    int32_t now, timeout, count, i, idx;
    Poller_EventBuf buf;
    Poller_Status pst;
    Timers_Status tst;
    Scheduler_Status sst;
    int didWork;
#line 244
    if ((lp == NULL)) {
        return 0;
    }
#line 245
    p = lp;
#line 247
    now = Poller_NowMs();
#line 250
    timeout = Timers_NextDeadline(p->timers, now);
#line 253
    if (((timeout < 0) && (p->nWatchers == 0))) {
#line 254
        sst = Scheduler_SchedulerPump(p->sched, 256, &didWork);
#line 255
        return didWork;
    }
#line 259
    if ((p->nWatchers > 0)) {
#line 260
        if ((timeout < 0)) {
#line 262
            timeout = 100;
        }
#line 264
        pst = Poller_Wait(p->poller, timeout, &buf, &count);
#line 267
        if ((count > 0)) {
#line 268
            for (i = 0; i <= (count - 1); i += 1) {
#line 269
                idx = EventLoop_FindWatcher(p, buf[i].fd);
#line 270
                if ((idx >= 0)) {
#line 271
                    p->watchers[idx].cb(buf[i].fd, buf[i].events, p->watchers[idx].user);
                }
            }
        }
    } else {
#line 278
        if ((timeout > 0)) {
#line 279
            pst = Poller_Wait(p->poller, timeout, &buf, &count);
        }
    }
#line 284
    now = Poller_NowMs();
#line 285
    tst = Timers_Tick(p->timers, now);
#line 288
    didWork = 1;
#line 289
    while (didWork) {
#line 290
        sst = Scheduler_SchedulerPump(p->sched, 256, &didWork);
    }
#line 294
    return ((p->nWatchers > 0) || (Timers_ActiveCount(p->timers) > 0));
}

static void EventLoop_Run(EventLoop_Loop lp) {
    EventLoop_LoopPtr p;
    int hasWork;
#line 301
    if ((lp == NULL)) {
        return;
    }
#line 302
    p = lp;
#line 303
    p->running = 1;
#line 304
    p->stopFlag = 0;
#line 305
    for (;;) {
#line 306
        hasWork = EventLoop_RunOnce(lp);
#line 307
        if (p->stopFlag) {
            break;
        }
#line 308
        if ((!hasWork)) {
            break;
        }
    }
#line 310
    p->running = 0;
}

static void EventLoop_Stop(EventLoop_Loop lp) {
    EventLoop_LoopPtr p;
#line 316
    if ((lp == NULL)) {
        return;
    }
#line 317
    p = lp;
#line 318
    p->stopFlag = 1;
}

/* Imported Module RpcClient */

typedef struct RpcClient_TimeoutCtx RpcClient_TimeoutCtx;
typedef struct RpcClient_PendingCall RpcClient_PendingCall;
typedef struct RpcClient_Client RpcClient_Client;
static const int32_t RpcClient_MaxInflight = 64;
struct RpcClient_TimeoutCtx {
    void * clientPtr;
    uint32_t requestId;
};

struct RpcClient_PendingCall {
    int active;
    uint32_t requestId;
    void * promise;
    Timers_TimerId timerId;
    int hasTimer;
    RpcClient_TimeoutCtx timeoutCtx;
};

struct RpcClient_Client {
    RpcFrame_ReadFn readFn;
    void * readCtx;
    RpcFrame_WriteFn writeFn;
    void * writeCtx;
    void * loop;
    Scheduler_Scheduler sched;
    uint32_t nextId;
    RpcClient_PendingCall pending[63 + 1];
    ByteBuf_Buf outBuf;
    ByteBuf_Buf respBuf;
    int alive;
};

static int32_t RpcClient_FindSlot(RpcClient_Client *c);
static int32_t RpcClient_FindPending(RpcClient_Client *c, uint32_t reqId);
static void RpcClient_RejectPending(RpcClient_Client *c, uint32_t idx, uint32_t code);
static void RpcClient_OnTimeout(void * user);
static void RpcClient_InitClient(RpcClient_Client *c, RpcFrame_ReadFn readFn, void * readCtx, RpcFrame_WriteFn writeFn, void * writeCtx, Scheduler_Scheduler sched, void * loop);
static void RpcClient_Call(RpcClient_Client *c, char *method, uint32_t method_high, uint32_t methodLen, ByteBuf_BytesView body, uint32_t timeoutMs, Promise_Future *out, int *ok);
static int RpcClient_OnReadable(RpcClient_Client *c);
static void RpcClient_CancelAll(RpcClient_Client *c);
static void RpcClient_FreeClient(RpcClient_Client *c);

RpcFrame_FrameReader RpcClient_frameRdr;
int RpcClient_frameRdrInit;
static int32_t RpcClient_FindSlot(RpcClient_Client *c) {
    uint32_t i;
#line 31 "libs/m2rpc/src/RpcClient.mod"
    i = 0;
#line 32
    while ((i < RpcClient_MaxInflight)) {
#line 33
        if ((!(*c).pending[i].active)) {
            return ((int32_t)(i));
        }
#line 34
        (i++);
    }
#line 36
    return (-1);
}

static int32_t RpcClient_FindPending(RpcClient_Client *c, uint32_t reqId) {
    uint32_t i;
#line 42
    i = 0;
#line 43
    while ((i < RpcClient_MaxInflight)) {
#line 44
        if (((*c).pending[i].active && ((*c).pending[i].requestId == reqId))) {
#line 45
            return ((int32_t)(i));
        }
#line 47
        (i++);
    }
#line 49
    return (-1);
}

static void RpcClient_RejectPending(RpcClient_Client *c, uint32_t idx, uint32_t code) {
    Promise_Error e;
    uint32_t st;
#line 57
    Promise_MakeError(((int32_t)(code)), NULL, &e);
#line 58
    st = ((uint32_t)(Promise_Reject((*c).pending[idx].promise, e)));
#line 59
    if (((*c).pending[idx].hasTimer && ((*c).loop != NULL))) {
#line 60
        st = ((uint32_t)(EventLoop_CancelTimer((*c).loop, (*c).pending[idx].timerId)));
    }
#line 62
    (*c).pending[idx].active = 0;
}

static void RpcClient_OnTimeout(void * user) {
    RpcClient_TimeoutCtx * tcp;
    RpcClient_Client * cp;
    int32_t idx;
#line 73
    tcp = user;
#line 74
    cp = tcp->clientPtr;
#line 75
    idx = RpcClient_FindPending(&(*cp), tcp->requestId);
#line 76
    if ((idx >= 0)) {
#line 77
        RpcClient_RejectPending(&(*cp), ((uint32_t)(idx)), RpcErrors_Timeout);
    }
}

static void RpcClient_InitClient(RpcClient_Client *c, RpcFrame_ReadFn readFn, void * readCtx, RpcFrame_WriteFn writeFn, void * writeCtx, Scheduler_Scheduler sched, void * loop) {
    uint32_t i;
#line 90
    (*c).readFn = readFn;
#line 91
    (*c).readCtx = readCtx;
#line 92
    (*c).writeFn = writeFn;
#line 93
    (*c).writeCtx = writeCtx;
#line 94
    (*c).loop = loop;
#line 95
    (*c).sched = sched;
#line 96
    (*c).nextId = 1;
#line 97
    (*c).alive = 1;
#line 98
    i = 0;
#line 99
    while ((i < RpcClient_MaxInflight)) {
#line 100
        (*c).pending[i].active = 0;
#line 101
        (i++);
    }
#line 103
    ByteBuf_Init(&(*c).outBuf, 256);
#line 104
    ByteBuf_Init(&(*c).respBuf, 256);
#line 105
    RpcFrame_InitFrameReader(&RpcClient_frameRdr, RpcFrame_MaxFrame, readFn, readCtx);
#line 106
    RpcClient_frameRdrInit = 1;
}

static void RpcClient_Call(RpcClient_Client *c, char *method, uint32_t method_high, uint32_t methodLen, ByteBuf_BytesView body, uint32_t timeoutMs, Promise_Future *out, int *ok) {
    int32_t slot;
    uint32_t reqId, st;
    Promise_Promise p;
    Promise_Future f;
    ByteBuf_BytesView pv;
    Timers_TimerId tid;
    RpcFrame_WriteFn wfn;
#line 125
    (*ok) = 0;
#line 126
    (*out) = NULL;
#line 128
    slot = RpcClient_FindSlot(c);
#line 129
    if ((slot < 0)) {
        return;
    }
#line 131
    reqId = (*c).nextId;
#line 132
    ((*c).nextId++);
#line 135
    if ((Promise_PromiseCreate((*c).sched, &p, &f) != Scheduler_Status_OK)) {
        return;
    }
#line 138
    ByteBuf_Clear(&(*c).outBuf);
#line 139
    RpcCodec_EncodeRequest(&(*c).outBuf, reqId, method, method_high, methodLen, body);
#line 140
    pv = ByteBuf_AsView(&(*c).outBuf);
#line 141
    wfn = (*c).writeFn;
#line 142
    RpcFrame_WriteFrame(wfn, (*c).writeCtx, pv, ok);
#line 143
    if ((!(*ok))) {
        return;
    }
#line 146
    (*c).pending[slot].active = 1;
#line 147
    (*c).pending[slot].requestId = reqId;
#line 148
    (*c).pending[slot].promise = p;
#line 149
    (*c).pending[slot].hasTimer = 0;
#line 150
    (*c).pending[slot].timeoutCtx.clientPtr = ((void *)&((*c)));
#line 151
    (*c).pending[slot].timeoutCtx.requestId = reqId;
#line 154
    if (((timeoutMs > 0) && ((*c).loop != NULL))) {
#line 155
        st = ((uint32_t)(EventLoop_SetTimeout((*c).loop, ((int32_t)(timeoutMs)), RpcClient_OnTimeout, ((void *)&((*c).pending[slot].timeoutCtx)), &tid)));
#line 159
        if ((st == ((uint32_t)(EventLoop_Status_OK)))) {
#line 160
            (*c).pending[slot].timerId = tid;
#line 161
            (*c).pending[slot].hasTimer = 1;
        }
    }
#line 165
    (*out) = f;
#line 166
    (*ok) = 1;
}

static int RpcClient_OnReadable(RpcClient_Client *c) {
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
    uint32_t ver, mt, reqId, errCode, st;
    ByteBuf_BytesView body, errMsg;
    int ok;
    int32_t idx;
    Promise_Value v;
#line 179
    if ((!(*c).alive)) {
        return 0;
    }
#line 181
    for (;;) {
#line 182
        RpcFrame_TryReadFrame(&RpcClient_frameRdr, &payload, &status);
#line 183
        if ((status == RpcFrame_FrameStatus_FrmOk)) {
#line 184
            RpcCodec_DecodeHeader(payload, &ver, &mt, &reqId, &ok);
#line 185
            if ((!ok)) {
#line 187
            } else if ((mt == RpcCodec_MsgResponse)) {
#line 188
                RpcCodec_DecodeResponse(payload, &reqId, &body, &ok);
#line 189
                if (ok) {
#line 190
                    idx = RpcClient_FindPending(c, reqId);
#line 191
                    if ((idx >= 0)) {
#line 193
                        ByteBuf_Clear(&(*c).respBuf);
#line 194
                        if ((body.len > 0)) {
#line 195
                            ByteBuf_AppendView(&(*c).respBuf, body);
                        }
#line 198
                        Promise_MakeValue(0, ((void *)&((*c).respBuf)), &v);
#line 199
                        st = ((uint32_t)(Promise_Resolve((*c).pending[idx].promise, v)));
#line 200
                        if (((*c).pending[idx].hasTimer && ((*c).loop != NULL))) {
#line 201
                            st = ((uint32_t)(EventLoop_CancelTimer((*c).loop, (*c).pending[idx].timerId)));
                        }
#line 204
                        (*c).pending[idx].active = 0;
                    }
                }
            } else if ((mt == RpcCodec_MsgError)) {
#line 208
                RpcCodec_DecodeError(payload, &reqId, &errCode, &errMsg, &body, &ok);
#line 209
                if (ok) {
#line 210
                    idx = RpcClient_FindPending(c, reqId);
#line 211
                    if ((idx >= 0)) {
#line 212
                        RpcClient_RejectPending(c, ((uint32_t)(idx)), errCode);
                    }
                }
            }
        } else if ((status == RpcFrame_FrameStatus_FrmNeedMore)) {
#line 217
            return 1;
        } else if ((status == RpcFrame_FrameStatus_FrmClosed)) {
#line 219
            (*c).alive = 0;
#line 220
            RpcClient_CancelAll(c);
#line 221
            return 0;
        } else {
#line 223
            (*c).alive = 0;
#line 224
            RpcClient_CancelAll(c);
#line 225
            return 0;
        }
    }
}

static void RpcClient_CancelAll(RpcClient_Client *c) {
    uint32_t i;
#line 233
    i = 0;
#line 234
    while ((i < RpcClient_MaxInflight)) {
#line 235
        if ((*c).pending[i].active) {
#line 236
            RpcClient_RejectPending(c, i, RpcErrors_Closed);
        }
#line 238
        (i++);
    }
}

static void RpcClient_FreeClient(RpcClient_Client *c) {
#line 244
    if (RpcClient_frameRdrInit) {
#line 245
        RpcFrame_FreeFrameReader(&RpcClient_frameRdr);
#line 246
        RpcClient_frameRdrInit = 0;
    }
#line 248
    ByteBuf_Free(&(*c).outBuf);
#line 249
    ByteBuf_Free(&(*c).respBuf);
}

static void RpcClient_init(void) {
#line 253
    RpcClient_frameRdrInit = 0;
}

/* Imported Module RpcServer */

typedef struct RpcServer_HandlerEntry RpcServer_HandlerEntry;
typedef struct RpcServer_Server RpcServer_Server;
static const int32_t RpcServer_MaxHandlers = 32;
static const int32_t RpcServer_MaxMethodLen = 64;
typedef void (*RpcServer_Handler)(void *, uint32_t, void *, uint32_t, ByteBuf_BytesView, ByteBuf_Buf *, uint32_t *, int *);

struct RpcServer_HandlerEntry {
    char method[63 + 1];
    uint32_t methodLen;
    RpcServer_Handler handler;
    void * ctx;
    int active;
};

struct RpcServer_Server {
    RpcFrame_FrameReader frameReader;
    RpcFrame_WriteFn writeFn;
    void * writeCtx;
    RpcServer_HandlerEntry handlers[31 + 1];
    uint32_t handlerCount;
    ByteBuf_Buf outBuf;
    ByteBuf_Buf respBuf;
};

static int RpcServer_StrEqual(char *a, uint32_t a_high, uint32_t aLen, ByteBuf_BytesView v);
static void RpcServer_InitServer(RpcServer_Server *s, RpcFrame_ReadFn readFn, void * readCtx, RpcFrame_WriteFn writeFn, void * writeCtx);
static int RpcServer_RegisterHandler(RpcServer_Server *s, char *method, uint32_t method_high, uint32_t methodLen, RpcServer_Handler handler, void * ctx);
static int32_t RpcServer_FindHandler(RpcServer_Server *s, ByteBuf_BytesView methodView);
static void RpcServer_DispatchHandler(RpcServer_Server *s, uint32_t idx, uint32_t reqId, ByteBuf_BytesView method, ByteBuf_BytesView body, uint32_t *errCode, int *handlerOk);
static void RpcServer_HandleRequest(RpcServer_Server *s, ByteBuf_BytesView payload);
static int RpcServer_ServeOnce(RpcServer_Server *s);
static void RpcServer_FreeServer(RpcServer_Server *s);

static int RpcServer_StrEqual(char *a, uint32_t a_high, uint32_t aLen, ByteBuf_BytesView v) {
    uint32_t i;
#line 23 "libs/m2rpc/src/RpcServer.mod"
    if ((aLen != v.len)) {
        return 0;
    }
#line 24
    i = 0;
#line 25
    while ((i < aLen)) {
#line 26
        if ((((uint32_t)((unsigned char)(a[i]))) != ByteBuf_ViewGetByte(v, i))) {
            return 0;
        }
#line 27
        (i++);
    }
#line 29
    return 1;
}

static void RpcServer_InitServer(RpcServer_Server *s, RpcFrame_ReadFn readFn, void * readCtx, RpcFrame_WriteFn writeFn, void * writeCtx) {
    uint32_t i;
#line 39
    RpcFrame_InitFrameReader(&(*s).frameReader, RpcFrame_MaxFrame, readFn, readCtx);
#line 40
    (*s).writeFn = writeFn;
#line 41
    (*s).writeCtx = writeCtx;
#line 42
    (*s).handlerCount = 0;
#line 43
    i = 0;
#line 44
    while ((i < RpcServer_MaxHandlers)) {
#line 45
        (*s).handlers[i].active = 0;
#line 46
        (i++);
    }
#line 48
    ByteBuf_Init(&(*s).outBuf, 256);
#line 49
    ByteBuf_Init(&(*s).respBuf, 256);
}

static int RpcServer_RegisterHandler(RpcServer_Server *s, char *method, uint32_t method_high, uint32_t methodLen, RpcServer_Handler handler, void * ctx) {
    uint32_t idx, ml;
#line 59
    if (((*s).handlerCount >= RpcServer_MaxHandlers)) {
        return 0;
    }
#line 60
    idx = (*s).handlerCount;
#line 61
    ml = methodLen;
#line 62
    if ((ml > RpcServer_MaxMethodLen)) {
        ml = RpcServer_MaxMethodLen;
    }
#line 63
    m2_Strings_Assign(method, (*s).handlers[idx].method, (sizeof((*s).handlers[idx].method) / sizeof((*s).handlers[idx].method[0])) - 1);
#line 64
    (*s).handlers[idx].methodLen = ml;
#line 65
    (*s).handlers[idx].handler = handler;
#line 66
    (*s).handlers[idx].ctx = ctx;
#line 67
    (*s).handlers[idx].active = 1;
#line 68
    ((*s).handlerCount++);
#line 69
    return 1;
}

static int32_t RpcServer_FindHandler(RpcServer_Server *s, ByteBuf_BytesView methodView) {
    uint32_t i;
#line 75
    i = 0;
#line 76
    while ((i < (*s).handlerCount)) {
#line 77
        if ((*s).handlers[i].active) {
#line 78
            if (RpcServer_StrEqual((*s).handlers[i].method, (sizeof((*s).handlers[i].method) / sizeof((*s).handlers[i].method[0])) - 1, (*s).handlers[i].methodLen, methodView)) {
#line 81
                return ((int32_t)(i));
            }
        }
#line 84
        (i++);
    }
#line 86
    return (-1);
}

static void RpcServer_DispatchHandler(RpcServer_Server *s, uint32_t idx, uint32_t reqId, ByteBuf_BytesView method, ByteBuf_BytesView body, uint32_t *errCode, int *handlerOk) {
    RpcServer_Handler h;
#line 97
    h = (*s).handlers[idx].handler;
#line 98
    h((*s).handlers[idx].ctx, reqId, method.base, method.len, body, &(*s).respBuf, errCode, handlerOk);
}

static void RpcServer_HandleRequest(RpcServer_Server *s, ByteBuf_BytesView payload) {
    uint32_t reqId, errCode;
    ByteBuf_BytesView method, body;
    int ok, handlerOk;
    int32_t idx;
    ByteBuf_BytesView respView;
    ByteBuf_BytesView emptyView;
#line 111
    emptyView.base = NULL;
#line 112
    emptyView.len = 0;
#line 114
    RpcCodec_DecodeRequest(payload, &reqId, &method, &body, &ok);
#line 115
    if ((!ok)) {
#line 116
        ByteBuf_Clear(&(*s).outBuf);
#line 117
        RpcCodec_EncodeError(&(*s).outBuf, 0, RpcErrors_BadRequest, "bad request", (sizeof("bad request") / sizeof("bad request"[0])) - 1, 11, emptyView);
#line 118
        respView = ByteBuf_AsView(&(*s).outBuf);
#line 119
        RpcFrame_WriteFrame((*s).writeFn, (*s).writeCtx, respView, &ok);
#line 120
        return;
    }
#line 123
    idx = RpcServer_FindHandler(s, method);
#line 124
    if ((idx < 0)) {
#line 125
        ByteBuf_Clear(&(*s).outBuf);
#line 126
        RpcCodec_EncodeError(&(*s).outBuf, reqId, RpcErrors_UnknownMethod, "unknown method", (sizeof("unknown method") / sizeof("unknown method"[0])) - 1, 14, emptyView);
#line 128
        respView = ByteBuf_AsView(&(*s).outBuf);
#line 129
        RpcFrame_WriteFrame((*s).writeFn, (*s).writeCtx, respView, &ok);
#line 130
        return;
    }
#line 133
    ByteBuf_Clear(&(*s).respBuf);
#line 134
    errCode = 0;
#line 135
    handlerOk = 1;
#line 136
    RpcServer_DispatchHandler(s, ((uint32_t)(idx)), reqId, method, body, &errCode, &handlerOk);
#line 139
    ByteBuf_Clear(&(*s).outBuf);
#line 140
    if (handlerOk) {
#line 141
        respView = ByteBuf_AsView(&(*s).respBuf);
#line 142
        RpcCodec_EncodeResponse(&(*s).outBuf, reqId, respView);
    } else {
#line 144
        respView = ByteBuf_AsView(&(*s).respBuf);
#line 145
        RpcCodec_EncodeError(&(*s).outBuf, reqId, errCode, "", (sizeof("") / sizeof(""[0])) - 1, 0, respView);
    }
#line 148
    respView = ByteBuf_AsView(&(*s).outBuf);
#line 149
    RpcFrame_WriteFrame((*s).writeFn, (*s).writeCtx, respView, &ok);
}

static int RpcServer_ServeOnce(RpcServer_Server *s) {
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
    uint32_t ver, mt, reqId;
    int ok;
    ByteBuf_BytesView respView, emptyView;
#line 160
    emptyView.base = NULL;
#line 161
    emptyView.len = 0;
#line 163
    for (;;) {
#line 164
        RpcFrame_TryReadFrame(&(*s).frameReader, &payload, &status);
#line 165
        if ((status == RpcFrame_FrameStatus_FrmOk)) {
#line 166
            RpcCodec_DecodeHeader(payload, &ver, &mt, &reqId, &ok);
#line 167
            if ((ok && (mt == RpcCodec_MsgRequest))) {
#line 168
                RpcServer_HandleRequest(s, payload);
            } else {
#line 170
                ByteBuf_Clear(&(*s).outBuf);
#line 171
                RpcCodec_EncodeError(&(*s).outBuf, reqId, RpcErrors_BadRequest, "expected request", (sizeof("expected request") / sizeof("expected request"[0])) - 1, 16, emptyView);
#line 173
                respView = ByteBuf_AsView(&(*s).outBuf);
#line 174
                RpcFrame_WriteFrame((*s).writeFn, (*s).writeCtx, respView, &ok);
            }
        } else if ((status == RpcFrame_FrameStatus_FrmNeedMore)) {
#line 177
            return 1;
        } else if ((status == RpcFrame_FrameStatus_FrmClosed)) {
#line 179
            return 0;
        } else {
#line 181
            return 0;
        }
    }
}

static void RpcServer_FreeServer(RpcServer_Server *s) {
#line 188
    RpcFrame_FreeFrameReader(&(*s).frameReader);
#line 189
    ByteBuf_Free(&(*s).outBuf);
#line 190
    ByteBuf_Free(&(*s).respBuf);
}

/* Imported Module RpcTest */

typedef struct RpcTest_PipeRec RpcTest_PipeRec;
typedef void * RpcTest_Pipe;

typedef char *RpcTest_CharPtr;

static const int32_t RpcTest_TsOk = 0;
static const int32_t RpcTest_TsWouldBlock = 1;
static const int32_t RpcTest_TsClosed = 2;
static const int32_t RpcTest_TsError = 3;
struct RpcTest_PipeRec {
    ByteBuf_Buf aToB;
    uint32_t aToBPos;
    int aClosed;
    ByteBuf_Buf bToA;
    uint32_t bToAPos;
    int bClosed;
    uint32_t readLimit;
    uint32_t writeLimit;
};

typedef RpcTest_PipeRec *RpcTest_PipePtr;

static void RpcTest_CreatePipe(RpcTest_Pipe *p, uint32_t readLimit, uint32_t writeLimit);
static void RpcTest_DestroyPipe(RpcTest_Pipe *p);
static void RpcTest_CloseA(RpcTest_Pipe p);
static void RpcTest_CloseB(RpcTest_Pipe p);
static uint32_t RpcTest_DoRead(ByteBuf_Buf *src, uint32_t *srcPos, int closed, uint32_t limit, void * buf, uint32_t max, uint32_t *got);
static uint32_t RpcTest_DoWrite(ByteBuf_Buf *dst, int closed, uint32_t limit, void * buf, uint32_t len, uint32_t *sent);
static uint32_t RpcTest_ReadA(void * ctx, void * buf, uint32_t max, uint32_t *got);
static uint32_t RpcTest_WriteA(void * ctx, void * buf, uint32_t len, uint32_t *sent);
static uint32_t RpcTest_ReadB(void * ctx, void * buf, uint32_t max, uint32_t *got);
static uint32_t RpcTest_WriteB(void * ctx, void * buf, uint32_t len, uint32_t *sent);
static uint32_t RpcTest_PendingAtoB(RpcTest_Pipe p);
static uint32_t RpcTest_PendingBtoA(RpcTest_Pipe p);

static void RpcTest_CreatePipe(RpcTest_Pipe *p, uint32_t readLimit, uint32_t writeLimit) {
    RpcTest_PipePtr pp;
#line 37 "libs/m2rpc/src/RpcTest.mod"
    m2_ALLOCATE(&pp, ((uint32_t)sizeof(RpcTest_PipeRec)));
#line 38
    ByteBuf_Init(&pp->aToB, 256);
#line 39
    pp->aToBPos = 0;
#line 40
    pp->aClosed = 0;
#line 41
    ByteBuf_Init(&pp->bToA, 256);
#line 42
    pp->bToAPos = 0;
#line 43
    pp->bClosed = 0;
#line 44
    pp->readLimit = readLimit;
#line 45
    pp->writeLimit = writeLimit;
#line 46
    (*p) = pp;
}

static void RpcTest_DestroyPipe(RpcTest_Pipe *p) {
    RpcTest_PipePtr pp;
#line 52
    if (((*p) == NULL)) {
        return;
    }
#line 53
    pp = (*p);
#line 54
    ByteBuf_Free(&pp->aToB);
#line 55
    ByteBuf_Free(&pp->bToA);
#line 56
    m2_DEALLOCATE(&pp, ((uint32_t)sizeof(RpcTest_PipeRec)));
#line 57
    (*p) = NULL;
}

static void RpcTest_CloseA(RpcTest_Pipe p) {
    RpcTest_PipePtr pp;
#line 63
    pp = p;
#line 64
    pp->aClosed = 1;
}

static void RpcTest_CloseB(RpcTest_Pipe p) {
    RpcTest_PipePtr pp;
#line 70
    pp = p;
#line 71
    pp->bClosed = 1;
}

static uint32_t RpcTest_DoRead(ByteBuf_Buf *src, uint32_t *srcPos, int closed, uint32_t limit, void * buf, uint32_t max, uint32_t *got) {
    uint32_t avail, n, i;
    RpcTest_CharPtr p;
#line 85
    (*got) = 0;
#line 86
    avail = ((*src).len - (*srcPos));
#line 87
    if ((avail == 0)) {
#line 88
        if (closed) {
            return RpcTest_TsClosed;
        }
#line 89
        return RpcTest_TsWouldBlock;
    }
#line 91
    n = max;
#line 92
    if ((n > avail)) {
        n = avail;
    }
#line 93
    if (((limit > 0) && (n > limit))) {
        n = limit;
    }
#line 94
    i = 0;
#line 95
    while ((i < n)) {
#line 96
        p = ((RpcTest_CharPtr)((((uint64_t)(buf)) + ((uint64_t)(i)))));
#line 97
        (*p) = ((char)(ByteBuf_GetByte(src, ((*srcPos) + i))));
#line 98
        (i++);
    }
#line 100
    (*srcPos) = ((*srcPos) + n);
#line 101
    (*got) = n;
#line 104
    if (((*srcPos) == (*src).len)) {
#line 105
        ByteBuf_Clear(src);
#line 106
        (*srcPos) = 0;
    }
#line 109
    return RpcTest_TsOk;
}

static uint32_t RpcTest_DoWrite(ByteBuf_Buf *dst, int closed, uint32_t limit, void * buf, uint32_t len, uint32_t *sent) {
    uint32_t n, i;
    RpcTest_CharPtr p;
#line 122
    (*sent) = 0;
#line 123
    if (closed) {
        return RpcTest_TsClosed;
    }
#line 124
    n = len;
#line 125
    if (((limit > 0) && (n > limit))) {
        n = limit;
    }
#line 126
    i = 0;
#line 127
    while ((i < n)) {
#line 128
        p = ((RpcTest_CharPtr)((((uint64_t)(buf)) + ((uint64_t)(i)))));
#line 129
        ByteBuf_AppendByte(dst, ((uint32_t)((unsigned char)((*p)))));
#line 130
        (i++);
    }
#line 132
    (*sent) = n;
#line 133
    return RpcTest_TsOk;
}

static uint32_t RpcTest_ReadA(void * ctx, void * buf, uint32_t max, uint32_t *got) {
    RpcTest_PipePtr pp;
#line 142
    pp = ctx;
#line 143
    return RpcTest_DoRead(&pp->bToA, &pp->bToAPos, pp->bClosed, pp->readLimit, buf, max, got);
}

static uint32_t RpcTest_WriteA(void * ctx, void * buf, uint32_t len, uint32_t *sent) {
    RpcTest_PipePtr pp;
#line 151
    pp = ctx;
#line 152
    return RpcTest_DoWrite(&pp->aToB, pp->aClosed, pp->writeLimit, buf, len, sent);
}

static uint32_t RpcTest_ReadB(void * ctx, void * buf, uint32_t max, uint32_t *got) {
    RpcTest_PipePtr pp;
#line 162
    pp = ctx;
#line 163
    return RpcTest_DoRead(&pp->aToB, &pp->aToBPos, pp->aClosed, pp->readLimit, buf, max, got);
}

static uint32_t RpcTest_WriteB(void * ctx, void * buf, uint32_t len, uint32_t *sent) {
    RpcTest_PipePtr pp;
#line 171
    pp = ctx;
#line 172
    return RpcTest_DoWrite(&pp->bToA, pp->bClosed, pp->writeLimit, buf, len, sent);
}

static uint32_t RpcTest_PendingAtoB(RpcTest_Pipe p) {
    RpcTest_PipePtr pp;
#line 181
    pp = p;
#line 182
    return (pp->aToB.len - pp->aToBPos);
}

static uint32_t RpcTest_PendingBtoA(RpcTest_Pipe p) {
    RpcTest_PipePtr pp;
#line 188
    pp = p;
#line 189
    return (pp->bToA.len - pp->bToAPos);
}

/* Module RpcTests */

void Check(char *name, uint32_t name_high, int cond);
void PumpSched(Scheduler_Scheduler s);
void WriteBytesToPipe(RpcTest_Pipe m2_pipe, ByteBuf_Buf *buf);
void TestFrameComplete(void);
void TestFrameSplitHeader(void);
void TestFrameSplitPayload(void);
void TestFrameTooLarge(void);
void TestFrameZeroLen(void);
void TestFrameClosedHeader(void);
void TestWriteFrameRoundtrip(void);
void TestCodecRequest(void);
void TestCodecResponse(void);
void TestCodecError(void);
void TestCodecTruncated(void);
void TestCodecBadVersion(void);
void TestCodecBadType(void);
void TestCodecEmptyBody(void);
void TestPipeBasic(void);
void TestPipePartialRead(void);
void TestPipePartialWrite(void);
void TestPipeClose(void);
void TestPipeBidir(void);
void PingHandler(void * ctx, uint32_t reqId, void * methodPtr, uint32_t methodLen, ByteBuf_BytesView body, ByteBuf_Buf *outBody, uint32_t *errCode, int *ok);
void EchoHandler(void * ctx, uint32_t reqId, void * methodPtr, uint32_t methodLen, ByteBuf_BytesView body, ByteBuf_Buf *outBody, uint32_t *errCode, int *ok);
void TestServerPing(void);
void TestServerUnknown(void);
void TestClientServerBasic(void);
void TestClientServerSequential(void);
void TestConcurrent20(void);
void TestErrorStrings(void);

int32_t passed, failed, total;

#line 39 "libs/m2rpc/tests/rpc_tests.mod"
void Check(char *name, uint32_t name_high, int cond) {
#line 41
    (total++);
#line 42
    if (cond) {
#line 43
        (passed++);
    } else {
#line 45
        (failed++);
#line 46
        m2_WriteString("FAIL: ");
        m2_WriteString(name);
        m2_WriteLn();
    }
}

#line 52
void PumpSched(Scheduler_Scheduler s) {
    int didWork;
    uint32_t st;
#line 55
    didWork = 1;
#line 56
    while (didWork) {
#line 57
        st = ((uint32_t)(Scheduler_SchedulerPump(s, 1000, &didWork)));
    }
}

#line 62
void WriteBytesToPipe(RpcTest_Pipe m2_pipe, ByteBuf_Buf *buf) {
    ByteBuf_BytesView v;
    uint32_t sent;
    uint32_t ts;
#line 65
    v = ByteBuf_AsView(buf);
#line 66
    if ((v.len > 0)) {
#line 67
        ts = RpcTest_WriteA(m2_pipe, v.base, v.len, &sent);
    }
}

#line 73
void TestFrameComplete(void) {
    RpcTest_Pipe m2_pipe;
    RpcFrame_FrameReader fr;
    ByteBuf_Buf frameBuf;
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
    uint32_t sent;
#line 82
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 83
    ByteBuf_Init(&frameBuf, 64);
#line 86
    ByteBuf_AppendByte(&frameBuf, 0);
#line 87
    ByteBuf_AppendByte(&frameBuf, 0);
#line 88
    ByteBuf_AppendByte(&frameBuf, 0);
#line 89
    ByteBuf_AppendByte(&frameBuf, 5);
#line 90
    ByteBuf_AppendByte(&frameBuf, ((uint32_t)((unsigned char)('H'))));
#line 91
    ByteBuf_AppendByte(&frameBuf, ((uint32_t)((unsigned char)('e'))));
#line 92
    ByteBuf_AppendByte(&frameBuf, ((uint32_t)((unsigned char)('l'))));
#line 93
    ByteBuf_AppendByte(&frameBuf, ((uint32_t)((unsigned char)('l'))));
#line 94
    ByteBuf_AppendByte(&frameBuf, ((uint32_t)((unsigned char)('o'))));
#line 96
    WriteBytesToPipe(m2_pipe, &frameBuf);
#line 98
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadB, m2_pipe);
#line 99
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 100
    Check("frame.complete: status=Ok", (sizeof("frame.complete: status=Ok") / sizeof("frame.complete: status=Ok"[0])) - 1, (status == RpcFrame_FrameStatus_FrmOk));
#line 101
    Check("frame.complete: len=5", (sizeof("frame.complete: len=5") / sizeof("frame.complete: len=5"[0])) - 1, (payload.len == 5));
#line 102
    Check("frame.complete: byte0=H", (sizeof("frame.complete: byte0=H") / sizeof("frame.complete: byte0=H"[0])) - 1, (ByteBuf_ViewGetByte(payload, 0) == ((uint32_t)((unsigned char)('H')))));
#line 103
    Check("frame.complete: byte4=o", (sizeof("frame.complete: byte4=o") / sizeof("frame.complete: byte4=o"[0])) - 1, (ByteBuf_ViewGetByte(payload, 4) == ((uint32_t)((unsigned char)('o')))));
#line 105
    RpcFrame_FreeFrameReader(&fr);
#line 106
    ByteBuf_Free(&frameBuf);
#line 107
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 112
void TestFrameSplitHeader(void) {
    RpcTest_Pipe m2_pipe;
    RpcFrame_FrameReader fr;
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
    char b[0 + 1];
    uint32_t sent;
#line 123
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 124
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadB, m2_pipe);
#line 127
    b[0] = ((char)(0));
#line 128
    sent = RpcTest_WriteA(m2_pipe, ((void *)&(b)), 1, &sent);
#line 129
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 130
    Check("frame.split: need1", (sizeof("frame.split: need1") / sizeof("frame.split: need1"[0])) - 1, (status == RpcFrame_FrameStatus_FrmNeedMore));
#line 133
    sent = RpcTest_WriteA(m2_pipe, ((void *)&(b)), 1, &sent);
#line 134
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 135
    Check("frame.split: need2", (sizeof("frame.split: need2") / sizeof("frame.split: need2"[0])) - 1, (status == RpcFrame_FrameStatus_FrmNeedMore));
#line 138
    sent = RpcTest_WriteA(m2_pipe, ((void *)&(b)), 1, &sent);
#line 139
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 140
    Check("frame.split: need3", (sizeof("frame.split: need3") / sizeof("frame.split: need3"[0])) - 1, (status == RpcFrame_FrameStatus_FrmNeedMore));
#line 143
    b[0] = ((char)(3));
#line 144
    sent = RpcTest_WriteA(m2_pipe, ((void *)&(b)), 1, &sent);
#line 145
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 146
    Check("frame.split: need4", (sizeof("frame.split: need4") / sizeof("frame.split: need4"[0])) - 1, (status == RpcFrame_FrameStatus_FrmNeedMore));
#line 149
    b[0] = 'a';
#line 150
    sent = RpcTest_WriteA(m2_pipe, ((void *)&(b)), 1, &sent);
#line 151
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 152
    Check("frame.split: need5", (sizeof("frame.split: need5") / sizeof("frame.split: need5"[0])) - 1, (status == RpcFrame_FrameStatus_FrmNeedMore));
#line 155
    b[0] = 'b';
#line 156
    sent = RpcTest_WriteA(m2_pipe, ((void *)&(b)), 1, &sent);
#line 157
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 158
    Check("frame.split: need6", (sizeof("frame.split: need6") / sizeof("frame.split: need6"[0])) - 1, (status == RpcFrame_FrameStatus_FrmNeedMore));
#line 161
    b[0] = 'c';
#line 162
    sent = RpcTest_WriteA(m2_pipe, ((void *)&(b)), 1, &sent);
#line 163
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 164
    Check("frame.split: ok", (sizeof("frame.split: ok") / sizeof("frame.split: ok"[0])) - 1, (status == RpcFrame_FrameStatus_FrmOk));
#line 165
    Check("frame.split: len=3", (sizeof("frame.split: len=3") / sizeof("frame.split: len=3"[0])) - 1, (payload.len == 3));
#line 166
    Check("frame.split: byte0=a", (sizeof("frame.split: byte0=a") / sizeof("frame.split: byte0=a"[0])) - 1, (ByteBuf_ViewGetByte(payload, 0) == ((uint32_t)((unsigned char)('a')))));
#line 168
    RpcFrame_FreeFrameReader(&fr);
#line 169
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 174
void TestFrameSplitPayload(void) {
    RpcTest_Pipe m2_pipe;
    RpcFrame_FrameReader fr;
    ByteBuf_Buf frameBuf;
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
    uint32_t i;
#line 183
    RpcTest_CreatePipe(&m2_pipe, 3, 0);
#line 184
    ByteBuf_Init(&frameBuf, 128);
#line 186
    ByteBuf_AppendByte(&frameBuf, 0);
#line 187
    ByteBuf_AppendByte(&frameBuf, 0);
#line 188
    ByteBuf_AppendByte(&frameBuf, 0);
#line 189
    ByteBuf_AppendByte(&frameBuf, 10);
#line 190
    i = 0;
#line 191
    while ((i < 10)) {
#line 192
        ByteBuf_AppendByte(&frameBuf, (65 + i));
#line 193
        (i++);
    }
#line 196
    WriteBytesToPipe(m2_pipe, &frameBuf);
#line 198
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadB, m2_pipe);
#line 200
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 201
    while ((status == RpcFrame_FrameStatus_FrmNeedMore)) {
#line 202
        RpcFrame_TryReadFrame(&fr, &payload, &status);
    }
#line 205
    Check("frame.payload_split: ok", (sizeof("frame.payload_split: ok") / sizeof("frame.payload_split: ok"[0])) - 1, (status == RpcFrame_FrameStatus_FrmOk));
#line 206
    Check("frame.payload_split: len=10", (sizeof("frame.payload_split: len=10") / sizeof("frame.payload_split: len=10"[0])) - 1, (payload.len == 10));
#line 207
    Check("frame.payload_split: first=A", (sizeof("frame.payload_split: first=A") / sizeof("frame.payload_split: first=A"[0])) - 1, (ByteBuf_ViewGetByte(payload, 0) == 65));
#line 208
    Check("frame.payload_split: last=J", (sizeof("frame.payload_split: last=J") / sizeof("frame.payload_split: last=J"[0])) - 1, (ByteBuf_ViewGetByte(payload, 9) == 74));
#line 210
    RpcFrame_FreeFrameReader(&fr);
#line 211
    ByteBuf_Free(&frameBuf);
#line 212
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 217
void TestFrameTooLarge(void) {
    RpcTest_Pipe m2_pipe;
    RpcFrame_FrameReader fr;
    ByteBuf_Buf frameBuf;
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
#line 225
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 226
    ByteBuf_Init(&frameBuf, 16);
#line 229
    ByteBuf_AppendByte(&frameBuf, 0);
#line 230
    ByteBuf_AppendByte(&frameBuf, 1);
#line 231
    ByteBuf_AppendByte(&frameBuf, 134);
#line 232
    ByteBuf_AppendByte(&frameBuf, 160);
#line 234
    WriteBytesToPipe(m2_pipe, &frameBuf);
#line 236
    RpcFrame_InitFrameReader(&fr, 100, RpcTest_ReadB, m2_pipe);
#line 237
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 238
    Check("frame.toolarge: rejected", (sizeof("frame.toolarge: rejected") / sizeof("frame.toolarge: rejected"[0])) - 1, (status == RpcFrame_FrameStatus_FrmTooLarge));
#line 240
    RpcFrame_FreeFrameReader(&fr);
#line 241
    ByteBuf_Free(&frameBuf);
#line 242
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 247
void TestFrameZeroLen(void) {
    RpcTest_Pipe m2_pipe;
    RpcFrame_FrameReader fr;
    ByteBuf_Buf frameBuf;
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
#line 255
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 256
    ByteBuf_Init(&frameBuf, 16);
#line 258
    ByteBuf_AppendByte(&frameBuf, 0);
#line 259
    ByteBuf_AppendByte(&frameBuf, 0);
#line 260
    ByteBuf_AppendByte(&frameBuf, 0);
#line 261
    ByteBuf_AppendByte(&frameBuf, 0);
#line 263
    WriteBytesToPipe(m2_pipe, &frameBuf);
#line 265
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadB, m2_pipe);
#line 266
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 267
    Check("frame.zerolen: ok", (sizeof("frame.zerolen: ok") / sizeof("frame.zerolen: ok"[0])) - 1, (status == RpcFrame_FrameStatus_FrmOk));
#line 268
    Check("frame.zerolen: len=0", (sizeof("frame.zerolen: len=0") / sizeof("frame.zerolen: len=0"[0])) - 1, (payload.len == 0));
#line 270
    RpcFrame_FreeFrameReader(&fr);
#line 271
    ByteBuf_Free(&frameBuf);
#line 272
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 277
void TestFrameClosedHeader(void) {
    RpcTest_Pipe m2_pipe;
    RpcFrame_FrameReader fr;
    ByteBuf_BytesView payload;
    RpcFrame_FrameStatus status;
#line 284
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 285
    RpcTest_CloseA(m2_pipe);
#line 287
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadB, m2_pipe);
#line 288
    RpcFrame_TryReadFrame(&fr, &payload, &status);
#line 289
    Check("frame.closed: detected", (sizeof("frame.closed: detected") / sizeof("frame.closed: detected"[0])) - 1, (status == RpcFrame_FrameStatus_FrmClosed));
#line 291
    RpcFrame_FreeFrameReader(&fr);
#line 292
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 297
void TestWriteFrameRoundtrip(void) {
    RpcTest_Pipe m2_pipe;
    RpcFrame_FrameReader fr;
    ByteBuf_Buf buf;
    ByteBuf_BytesView payload, out;
    RpcFrame_FrameStatus status;
    int ok;
#line 306
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 307
    ByteBuf_Init(&buf, 64);
#line 309
    ByteBuf_AppendByte(&buf, ((uint32_t)((unsigned char)('T'))));
#line 310
    ByteBuf_AppendByte(&buf, ((uint32_t)((unsigned char)('e'))));
#line 311
    ByteBuf_AppendByte(&buf, ((uint32_t)((unsigned char)('s'))));
#line 312
    ByteBuf_AppendByte(&buf, ((uint32_t)((unsigned char)('t'))));
#line 313
    payload = ByteBuf_AsView(&buf);
#line 314
    RpcFrame_WriteFrame(RpcTest_WriteA, m2_pipe, payload, &ok);
#line 315
    Check("writeframe: write ok", (sizeof("writeframe: write ok") / sizeof("writeframe: write ok"[0])) - 1, ok);
#line 317
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadB, m2_pipe);
#line 318
    RpcFrame_TryReadFrame(&fr, &out, &status);
#line 319
    Check("writeframe: read ok", (sizeof("writeframe: read ok") / sizeof("writeframe: read ok"[0])) - 1, (status == RpcFrame_FrameStatus_FrmOk));
#line 320
    Check("writeframe: len=4", (sizeof("writeframe: len=4") / sizeof("writeframe: len=4"[0])) - 1, (out.len == 4));
#line 321
    Check("writeframe: byte0=T", (sizeof("writeframe: byte0=T") / sizeof("writeframe: byte0=T"[0])) - 1, (ByteBuf_ViewGetByte(out, 0) == ((uint32_t)((unsigned char)('T')))));
#line 322
    Check("writeframe: byte3=t", (sizeof("writeframe: byte3=t") / sizeof("writeframe: byte3=t"[0])) - 1, (ByteBuf_ViewGetByte(out, 3) == ((uint32_t)((unsigned char)('t')))));
#line 324
    RpcFrame_FreeFrameReader(&fr);
#line 325
    ByteBuf_Free(&buf);
#line 326
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 331
void TestCodecRequest(void) {
    ByteBuf_Buf buf, bodyBuf;
    ByteBuf_BytesView payload, method, body;
    uint32_t reqId;
    int ok;
#line 338
    ByteBuf_Init(&buf, 256);
#line 339
    ByteBuf_Init(&bodyBuf, 64);
#line 341
    ByteBuf_AppendByte(&bodyBuf, ((uint32_t)((unsigned char)('x'))));
#line 342
    ByteBuf_AppendByte(&bodyBuf, ((uint32_t)((unsigned char)('y'))));
#line 343
    body = ByteBuf_AsView(&bodyBuf);
#line 345
    RpcCodec_EncodeRequest(&buf, 42, "Echo", (sizeof("Echo") / sizeof("Echo"[0])) - 1, 4, body);
#line 346
    payload = ByteBuf_AsView(&buf);
#line 348
    RpcCodec_DecodeRequest(payload, &reqId, &method, &body, &ok);
#line 349
    Check("codec.req: decode ok", (sizeof("codec.req: decode ok") / sizeof("codec.req: decode ok"[0])) - 1, ok);
#line 350
    Check("codec.req: reqId=42", (sizeof("codec.req: reqId=42") / sizeof("codec.req: reqId=42"[0])) - 1, (reqId == 42));
#line 351
    Check("codec.req: method len=4", (sizeof("codec.req: method len=4") / sizeof("codec.req: method len=4"[0])) - 1, (method.len == 4));
#line 352
    Check("codec.req: method[0]=E", (sizeof("codec.req: method[0]=E") / sizeof("codec.req: method[0]=E"[0])) - 1, (ByteBuf_ViewGetByte(method, 0) == ((uint32_t)((unsigned char)('E')))));
#line 353
    Check("codec.req: body len=2", (sizeof("codec.req: body len=2") / sizeof("codec.req: body len=2"[0])) - 1, (body.len == 2));
#line 354
    Check("codec.req: body[0]=x", (sizeof("codec.req: body[0]=x") / sizeof("codec.req: body[0]=x"[0])) - 1, (ByteBuf_ViewGetByte(body, 0) == ((uint32_t)((unsigned char)('x')))));
#line 356
    ByteBuf_Free(&buf);
#line 357
    ByteBuf_Free(&bodyBuf);
}

#line 362
void TestCodecResponse(void) {
    ByteBuf_Buf buf, bodyBuf;
    ByteBuf_BytesView payload, body;
    uint32_t reqId;
    int ok;
#line 369
    ByteBuf_Init(&buf, 256);
#line 370
    ByteBuf_Init(&bodyBuf, 64);
#line 372
    ByteBuf_AppendByte(&bodyBuf, ((uint32_t)((unsigned char)('O'))));
#line 373
    ByteBuf_AppendByte(&bodyBuf, ((uint32_t)((unsigned char)('K'))));
#line 374
    body = ByteBuf_AsView(&bodyBuf);
#line 376
    RpcCodec_EncodeResponse(&buf, 99, body);
#line 377
    payload = ByteBuf_AsView(&buf);
#line 379
    RpcCodec_DecodeResponse(payload, &reqId, &body, &ok);
#line 380
    Check("codec.resp: decode ok", (sizeof("codec.resp: decode ok") / sizeof("codec.resp: decode ok"[0])) - 1, ok);
#line 381
    Check("codec.resp: reqId=99", (sizeof("codec.resp: reqId=99") / sizeof("codec.resp: reqId=99"[0])) - 1, (reqId == 99));
#line 382
    Check("codec.resp: body len=2", (sizeof("codec.resp: body len=2") / sizeof("codec.resp: body len=2"[0])) - 1, (body.len == 2));
#line 383
    Check("codec.resp: body[0]=O", (sizeof("codec.resp: body[0]=O") / sizeof("codec.resp: body[0]=O"[0])) - 1, (ByteBuf_ViewGetByte(body, 0) == ((uint32_t)((unsigned char)('O')))));
#line 385
    ByteBuf_Free(&buf);
#line 386
    ByteBuf_Free(&bodyBuf);
}

#line 391
void TestCodecError(void) {
    ByteBuf_Buf buf;
    ByteBuf_BytesView payload, errMsg, body;
    ByteBuf_BytesView empty;
    uint32_t reqId, errCode;
    int ok;
#line 399
    ByteBuf_Init(&buf, 256);
#line 400
    empty.base = NULL;
#line 401
    empty.len = 0;
#line 403
    RpcCodec_EncodeError(&buf, 7, RpcErrors_UnknownMethod, "not found", (sizeof("not found") / sizeof("not found"[0])) - 1, 9, empty);
#line 404
    payload = ByteBuf_AsView(&buf);
#line 406
    RpcCodec_DecodeError(payload, &reqId, &errCode, &errMsg, &body, &ok);
#line 407
    Check("codec.err: decode ok", (sizeof("codec.err: decode ok") / sizeof("codec.err: decode ok"[0])) - 1, ok);
#line 408
    Check("codec.err: reqId=7", (sizeof("codec.err: reqId=7") / sizeof("codec.err: reqId=7"[0])) - 1, (reqId == 7));
#line 409
    Check("codec.err: code=UnknownMethod", (sizeof("codec.err: code=UnknownMethod") / sizeof("codec.err: code=UnknownMethod"[0])) - 1, (errCode == RpcErrors_UnknownMethod));
#line 410
    Check("codec.err: msg len=9", (sizeof("codec.err: msg len=9") / sizeof("codec.err: msg len=9"[0])) - 1, (errMsg.len == 9));
#line 411
    Check("codec.err: body empty", (sizeof("codec.err: body empty") / sizeof("codec.err: body empty"[0])) - 1, (body.len == 0));
#line 413
    ByteBuf_Free(&buf);
}

#line 418
void TestCodecTruncated(void) {
    ByteBuf_Buf buf;
    ByteBuf_BytesView payload, method, body;
    uint32_t reqId;
    int ok;
#line 425
    ByteBuf_Init(&buf, 16);
#line 426
    ByteBuf_AppendByte(&buf, 1);
#line 427
    ByteBuf_AppendByte(&buf, 0);
#line 428
    ByteBuf_AppendByte(&buf, 0);
#line 429
    payload = ByteBuf_AsView(&buf);
#line 431
    RpcCodec_DecodeRequest(payload, &reqId, &method, &body, &ok);
#line 432
    Check("codec.trunc: rejected", (sizeof("codec.trunc: rejected") / sizeof("codec.trunc: rejected"[0])) - 1, (!ok));
#line 434
    ByteBuf_Free(&buf);
}

#line 439
void TestCodecBadVersion(void) {
    ByteBuf_Buf buf;
    ByteBuf_BytesView payload, method, body;
    uint32_t reqId;
    int ok;
#line 446
    ByteBuf_Init(&buf, 64);
#line 447
    ByteBuf_AppendByte(&buf, 99);
#line 448
    ByteBuf_AppendByte(&buf, 0);
#line 449
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 450
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 1);
#line 451
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 452
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 453
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 454
    payload = ByteBuf_AsView(&buf);
#line 456
    RpcCodec_DecodeRequest(payload, &reqId, &method, &body, &ok);
#line 457
    Check("codec.badver: rejected", (sizeof("codec.badver: rejected") / sizeof("codec.badver: rejected"[0])) - 1, (!ok));
#line 459
    ByteBuf_Free(&buf);
}

#line 464
void TestCodecBadType(void) {
    ByteBuf_Buf buf;
    ByteBuf_BytesView payload, body;
    uint32_t reqId;
    int ok;
#line 471
    ByteBuf_Init(&buf, 64);
#line 472
    ByteBuf_AppendByte(&buf, 1);
#line 473
    ByteBuf_AppendByte(&buf, 0);
#line 474
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 475
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 1);
#line 476
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 477
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 478
    ByteBuf_AppendByte(&buf, 0);
    ByteBuf_AppendByte(&buf, 0);
#line 479
    payload = ByteBuf_AsView(&buf);
#line 482
    RpcCodec_DecodeResponse(payload, &reqId, &body, &ok);
#line 483
    Check("codec.badtype: rejected", (sizeof("codec.badtype: rejected") / sizeof("codec.badtype: rejected"[0])) - 1, (!ok));
#line 485
    ByteBuf_Free(&buf);
}

#line 490
void TestCodecEmptyBody(void) {
    ByteBuf_Buf buf;
    ByteBuf_BytesView payload, body;
    ByteBuf_BytesView empty;
    uint32_t reqId;
    int ok;
#line 498
    ByteBuf_Init(&buf, 64);
#line 499
    empty.base = NULL;
#line 500
    empty.len = 0;
#line 502
    RpcCodec_EncodeResponse(&buf, 55, empty);
#line 503
    payload = ByteBuf_AsView(&buf);
#line 505
    RpcCodec_DecodeResponse(payload, &reqId, &body, &ok);
#line 506
    Check("codec.empty: decode ok", (sizeof("codec.empty: decode ok") / sizeof("codec.empty: decode ok"[0])) - 1, ok);
#line 507
    Check("codec.empty: reqId=55", (sizeof("codec.empty: reqId=55") / sizeof("codec.empty: reqId=55"[0])) - 1, (reqId == 55));
#line 508
    Check("codec.empty: body empty", (sizeof("codec.empty: body empty") / sizeof("codec.empty: body empty"[0])) - 1, (body.len == 0));
#line 510
    ByteBuf_Free(&buf);
}

#line 515
void TestPipeBasic(void) {
    RpcTest_Pipe m2_pipe;
    char wbuf[7 + 1];
    char rbuf[7 + 1];
    uint32_t sent, got, ts;
#line 522
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 524
    wbuf[0] = 'H';
    wbuf[1] = 'i';
#line 525
    ts = RpcTest_WriteA(m2_pipe, ((void *)&(wbuf)), 2, &sent);
#line 526
    Check("pipe.basic: write ok", (sizeof("pipe.basic: write ok") / sizeof("pipe.basic: write ok"[0])) - 1, (ts == RpcFrame_TsOk));
#line 527
    Check("pipe.basic: sent=2", (sizeof("pipe.basic: sent=2") / sizeof("pipe.basic: sent=2"[0])) - 1, (sent == 2));
#line 528
    Check("pipe.basic: pending=2", (sizeof("pipe.basic: pending=2") / sizeof("pipe.basic: pending=2"[0])) - 1, (RpcTest_PendingAtoB(m2_pipe) == 2));
#line 530
    ts = RpcTest_ReadB(m2_pipe, ((void *)&(rbuf)), 8, &got);
#line 531
    Check("pipe.basic: read ok", (sizeof("pipe.basic: read ok") / sizeof("pipe.basic: read ok"[0])) - 1, (ts == RpcFrame_TsOk));
#line 532
    Check("pipe.basic: got=2", (sizeof("pipe.basic: got=2") / sizeof("pipe.basic: got=2"[0])) - 1, (got == 2));
#line 533
    Check("pipe.basic: byte0=H", (sizeof("pipe.basic: byte0=H") / sizeof("pipe.basic: byte0=H"[0])) - 1, (rbuf[0] == 'H'));
#line 535
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 540
void TestPipePartialRead(void) {
    RpcTest_Pipe m2_pipe;
    char wbuf[7 + 1];
    char rbuf[7 + 1];
    uint32_t sent, got, ts;
#line 547
    RpcTest_CreatePipe(&m2_pipe, 2, 0);
#line 549
    wbuf[0] = 'A';
    wbuf[1] = 'B';
#line 550
    wbuf[2] = 'C';
    wbuf[3] = 'D';
#line 551
    ts = RpcTest_WriteA(m2_pipe, ((void *)&(wbuf)), 4, &sent);
#line 552
    Check("pipe.partial_read: write 4", (sizeof("pipe.partial_read: write 4") / sizeof("pipe.partial_read: write 4"[0])) - 1, (sent == 4));
#line 554
    ts = RpcTest_ReadB(m2_pipe, ((void *)&(rbuf)), 8, &got);
#line 555
    Check("pipe.partial_read: got=2", (sizeof("pipe.partial_read: got=2") / sizeof("pipe.partial_read: got=2"[0])) - 1, (got == 2));
#line 556
    Check("pipe.partial_read: byte0=A", (sizeof("pipe.partial_read: byte0=A") / sizeof("pipe.partial_read: byte0=A"[0])) - 1, (rbuf[0] == 'A'));
#line 558
    ts = RpcTest_ReadB(m2_pipe, ((void *)&(rbuf)), 8, &got);
#line 559
    Check("pipe.partial_read: got=2b", (sizeof("pipe.partial_read: got=2b") / sizeof("pipe.partial_read: got=2b"[0])) - 1, (got == 2));
#line 560
    Check("pipe.partial_read: byte0=C", (sizeof("pipe.partial_read: byte0=C") / sizeof("pipe.partial_read: byte0=C"[0])) - 1, (rbuf[0] == 'C'));
#line 562
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 567
void TestPipePartialWrite(void) {
    RpcTest_Pipe m2_pipe;
    char wbuf[7 + 1];
    char rbuf[7 + 1];
    uint32_t sent, got, ts;
#line 574
    RpcTest_CreatePipe(&m2_pipe, 0, 3);
#line 576
    wbuf[0] = '1';
    wbuf[1] = '2';
#line 577
    wbuf[2] = '3';
    wbuf[3] = '4';
    wbuf[4] = '5';
#line 578
    ts = RpcTest_WriteA(m2_pipe, ((void *)&(wbuf)), 5, &sent);
#line 579
    Check("pipe.partial_write: sent=3", (sizeof("pipe.partial_write: sent=3") / sizeof("pipe.partial_write: sent=3"[0])) - 1, (sent == 3));
#line 581
    ts = RpcTest_ReadB(m2_pipe, ((void *)&(rbuf)), 8, &got);
#line 582
    Check("pipe.partial_write: got=3", (sizeof("pipe.partial_write: got=3") / sizeof("pipe.partial_write: got=3"[0])) - 1, (got == 3));
#line 583
    Check("pipe.partial_write: byte0=1", (sizeof("pipe.partial_write: byte0=1") / sizeof("pipe.partial_write: byte0=1"[0])) - 1, (rbuf[0] == '1'));
#line 585
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 590
void TestPipeClose(void) {
    RpcTest_Pipe m2_pipe;
    char rbuf[7 + 1];
    uint32_t got, ts;
#line 596
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 597
    RpcTest_CloseA(m2_pipe);
#line 599
    ts = RpcTest_ReadB(m2_pipe, ((void *)&(rbuf)), 8, &got);
#line 600
    Check("pipe.close: closed", (sizeof("pipe.close: closed") / sizeof("pipe.close: closed"[0])) - 1, (ts == RpcFrame_TsClosed));
#line 602
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 607
void TestPipeBidir(void) {
    RpcTest_Pipe m2_pipe;
    char wbuf[7 + 1], rbuf[7 + 1];
    uint32_t sent, got, ts;
#line 613
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 615
    wbuf[0] = 'X';
#line 616
    ts = RpcTest_WriteA(m2_pipe, ((void *)&(wbuf)), 1, &sent);
#line 617
    Check("pipe.bidir: A->B write", (sizeof("pipe.bidir: A->B write") / sizeof("pipe.bidir: A->B write"[0])) - 1, (sent == 1));
#line 619
    wbuf[0] = 'Y';
#line 620
    ts = RpcTest_WriteB(m2_pipe, ((void *)&(wbuf)), 1, &sent);
#line 621
    Check("pipe.bidir: B->A write", (sizeof("pipe.bidir: B->A write") / sizeof("pipe.bidir: B->A write"[0])) - 1, (sent == 1));
#line 623
    ts = RpcTest_ReadB(m2_pipe, ((void *)&(rbuf)), 8, &got);
#line 624
    Check("pipe.bidir: B reads X", (sizeof("pipe.bidir: B reads X") / sizeof("pipe.bidir: B reads X"[0])) - 1, ((got == 1) && (rbuf[0] == 'X')));
#line 626
    ts = RpcTest_ReadA(m2_pipe, ((void *)&(rbuf)), 8, &got);
#line 627
    Check("pipe.bidir: A reads Y", (sizeof("pipe.bidir: A reads Y") / sizeof("pipe.bidir: A reads Y"[0])) - 1, ((got == 1) && (rbuf[0] == 'Y')));
#line 629
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 635
void PingHandler(void * ctx, uint32_t reqId, void * methodPtr, uint32_t methodLen, ByteBuf_BytesView body, ByteBuf_Buf *outBody, uint32_t *errCode, int *ok) {
#line 641
    ByteBuf_Clear(outBody);
#line 642
    ByteBuf_AppendByte(outBody, ((uint32_t)((unsigned char)('P'))));
#line 643
    ByteBuf_AppendByte(outBody, ((uint32_t)((unsigned char)('o'))));
#line 644
    ByteBuf_AppendByte(outBody, ((uint32_t)((unsigned char)('n'))));
#line 645
    ByteBuf_AppendByte(outBody, ((uint32_t)((unsigned char)('g'))));
#line 646
    (*errCode) = 0;
#line 647
    (*ok) = 1;
}

#line 651
void EchoHandler(void * ctx, uint32_t reqId, void * methodPtr, uint32_t methodLen, ByteBuf_BytesView body, ByteBuf_Buf *outBody, uint32_t *errCode, int *ok) {
#line 657
    ByteBuf_Clear(outBody);
#line 658
    if ((body.len > 0)) {
#line 659
        ByteBuf_AppendView(outBody, body);
    }
#line 661
    (*errCode) = 0;
#line 662
    (*ok) = 1;
}

#line 667
void TestServerPing(void) {
    RpcTest_Pipe m2_pipe;
    RpcServer_Server srv;
    ByteBuf_Buf reqBuf;
    ByteBuf_BytesView reqView, respPayload, body, method;
    RpcFrame_FrameReader fr;
    RpcFrame_FrameStatus status;
    ByteBuf_BytesView empty;
    int ok;
    uint32_t reqId;
#line 679
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 680
    ByteBuf_Init(&reqBuf, 256);
#line 681
    empty.base = NULL;
#line 682
    empty.len = 0;
#line 684
    RpcServer_InitServer(&srv, RpcTest_ReadB, m2_pipe, RpcTest_WriteB, m2_pipe);
#line 685
    ok = RpcServer_RegisterHandler(&srv, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, PingHandler, NULL);
#line 686
    Check("srv.ping: register ok", (sizeof("srv.ping: register ok") / sizeof("srv.ping: register ok"[0])) - 1, ok);
#line 688
    RpcCodec_EncodeRequest(&reqBuf, 1, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, empty);
#line 689
    reqView = ByteBuf_AsView(&reqBuf);
#line 690
    RpcFrame_WriteFrame(RpcTest_WriteA, m2_pipe, reqView, &ok);
#line 691
    Check("srv.ping: write ok", (sizeof("srv.ping: write ok") / sizeof("srv.ping: write ok"[0])) - 1, ok);
#line 693
    ok = RpcServer_ServeOnce(&srv);
#line 695
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadA, m2_pipe);
#line 696
    RpcFrame_TryReadFrame(&fr, &respPayload, &status);
#line 697
    Check("srv.ping: resp frame ok", (sizeof("srv.ping: resp frame ok") / sizeof("srv.ping: resp frame ok"[0])) - 1, (status == RpcFrame_FrameStatus_FrmOk));
#line 699
    RpcCodec_DecodeResponse(respPayload, &reqId, &body, &ok);
#line 700
    Check("srv.ping: decode ok", (sizeof("srv.ping: decode ok") / sizeof("srv.ping: decode ok"[0])) - 1, ok);
#line 701
    Check("srv.ping: reqId=1", (sizeof("srv.ping: reqId=1") / sizeof("srv.ping: reqId=1"[0])) - 1, (reqId == 1));
#line 702
    Check("srv.ping: body len=4", (sizeof("srv.ping: body len=4") / sizeof("srv.ping: body len=4"[0])) - 1, (body.len == 4));
#line 703
    Check("srv.ping: body=Pong", (sizeof("srv.ping: body=Pong") / sizeof("srv.ping: body=Pong"[0])) - 1, ((ByteBuf_ViewGetByte(body, 0) == ((uint32_t)((unsigned char)('P')))) && (ByteBuf_ViewGetByte(body, 3) == ((uint32_t)((unsigned char)('g'))))));
#line 707
    RpcFrame_FreeFrameReader(&fr);
#line 708
    RpcServer_FreeServer(&srv);
#line 709
    ByteBuf_Free(&reqBuf);
#line 710
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 715
void TestServerUnknown(void) {
    RpcTest_Pipe m2_pipe;
    RpcServer_Server srv;
    ByteBuf_Buf reqBuf;
    ByteBuf_BytesView reqView, respPayload, errMsg, body;
    RpcFrame_FrameReader fr;
    RpcFrame_FrameStatus status;
    ByteBuf_BytesView empty;
    int ok;
    uint32_t reqId, errCode;
#line 727
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 728
    ByteBuf_Init(&reqBuf, 256);
#line 729
    empty.base = NULL;
#line 730
    empty.len = 0;
#line 732
    RpcServer_InitServer(&srv, RpcTest_ReadB, m2_pipe, RpcTest_WriteB, m2_pipe);
#line 734
    RpcCodec_EncodeRequest(&reqBuf, 5, "NoSuch", (sizeof("NoSuch") / sizeof("NoSuch"[0])) - 1, 6, empty);
#line 735
    reqView = ByteBuf_AsView(&reqBuf);
#line 736
    RpcFrame_WriteFrame(RpcTest_WriteA, m2_pipe, reqView, &ok);
#line 738
    ok = RpcServer_ServeOnce(&srv);
#line 740
    RpcFrame_InitFrameReader(&fr, RpcFrame_MaxFrame, RpcTest_ReadA, m2_pipe);
#line 741
    RpcFrame_TryReadFrame(&fr, &respPayload, &status);
#line 742
    Check("srv.unknown: frame ok", (sizeof("srv.unknown: frame ok") / sizeof("srv.unknown: frame ok"[0])) - 1, (status == RpcFrame_FrameStatus_FrmOk));
#line 744
    RpcCodec_DecodeError(respPayload, &reqId, &errCode, &errMsg, &body, &ok);
#line 745
    Check("srv.unknown: decode ok", (sizeof("srv.unknown: decode ok") / sizeof("srv.unknown: decode ok"[0])) - 1, ok);
#line 746
    Check("srv.unknown: reqId=5", (sizeof("srv.unknown: reqId=5") / sizeof("srv.unknown: reqId=5"[0])) - 1, (reqId == 5));
#line 747
    Check("srv.unknown: code=UnknownMethod", (sizeof("srv.unknown: code=UnknownMethod") / sizeof("srv.unknown: code=UnknownMethod"[0])) - 1, (errCode == RpcErrors_UnknownMethod));
#line 749
    RpcFrame_FreeFrameReader(&fr);
#line 750
    RpcServer_FreeServer(&srv);
#line 751
    ByteBuf_Free(&reqBuf);
#line 752
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 757
void TestClientServerBasic(void) {
    RpcTest_Pipe m2_pipe;
    RpcServer_Server srv;
    RpcClient_Client cli;
    Scheduler_Scheduler sched;
    Promise_Future f;
    Promise_Fate fate;
    ByteBuf_BytesView empty;
    int ok;
    uint32_t st;
#line 769
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 770
    st = ((uint32_t)(Scheduler_SchedulerCreate(256, &sched)));
#line 771
    empty.base = NULL;
#line 772
    empty.len = 0;
#line 774
    RpcServer_InitServer(&srv, RpcTest_ReadB, m2_pipe, RpcTest_WriteB, m2_pipe);
#line 775
    ok = RpcServer_RegisterHandler(&srv, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, PingHandler, NULL);
#line 777
    RpcClient_InitClient(&cli, RpcTest_ReadA, m2_pipe, RpcTest_WriteA, m2_pipe, sched, NULL);
#line 779
    RpcClient_Call(&cli, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, empty, 0, &f, &ok);
#line 780
    Check("cs.basic: call ok", (sizeof("cs.basic: call ok") / sizeof("cs.basic: call ok"[0])) - 1, ok);
#line 782
    ok = RpcServer_ServeOnce(&srv);
#line 783
    ok = RpcClient_OnReadable(&cli);
#line 784
    Check("cs.basic: alive", (sizeof("cs.basic: alive") / sizeof("cs.basic: alive"[0])) - 1, ok);
#line 786
    PumpSched(sched);
#line 788
    st = ((uint32_t)(Promise_GetFate(f, &fate)));
#line 789
    Check("cs.basic: fulfilled", (sizeof("cs.basic: fulfilled") / sizeof("cs.basic: fulfilled"[0])) - 1, (fate == Promise_Fate_Fulfilled));
#line 791
    RpcClient_FreeClient(&cli);
#line 792
    RpcServer_FreeServer(&srv);
#line 793
    st = ((uint32_t)(Scheduler_SchedulerDestroy(&sched)));
#line 794
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 799
void TestClientServerSequential(void) {
    RpcTest_Pipe m2_pipe;
    RpcServer_Server srv;
    RpcClient_Client cli;
    Scheduler_Scheduler sched;
    Promise_Future f1, f2, f3;
    Promise_Fate fate;
    ByteBuf_BytesView empty;
    int ok;
    uint32_t st;
#line 811
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 812
    st = ((uint32_t)(Scheduler_SchedulerCreate(256, &sched)));
#line 813
    empty.base = NULL;
#line 814
    empty.len = 0;
#line 816
    RpcServer_InitServer(&srv, RpcTest_ReadB, m2_pipe, RpcTest_WriteB, m2_pipe);
#line 817
    ok = RpcServer_RegisterHandler(&srv, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, PingHandler, NULL);
#line 819
    RpcClient_InitClient(&cli, RpcTest_ReadA, m2_pipe, RpcTest_WriteA, m2_pipe, sched, NULL);
#line 821
    RpcClient_Call(&cli, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, empty, 0, &f1, &ok);
#line 822
    Check("cs.seq: call1 ok", (sizeof("cs.seq: call1 ok") / sizeof("cs.seq: call1 ok"[0])) - 1, ok);
#line 823
    ok = RpcServer_ServeOnce(&srv);
#line 824
    ok = RpcClient_OnReadable(&cli);
#line 825
    PumpSched(sched);
#line 827
    RpcClient_Call(&cli, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, empty, 0, &f2, &ok);
#line 828
    Check("cs.seq: call2 ok", (sizeof("cs.seq: call2 ok") / sizeof("cs.seq: call2 ok"[0])) - 1, ok);
#line 829
    ok = RpcServer_ServeOnce(&srv);
#line 830
    ok = RpcClient_OnReadable(&cli);
#line 831
    PumpSched(sched);
#line 833
    RpcClient_Call(&cli, "Ping", (sizeof("Ping") / sizeof("Ping"[0])) - 1, 4, empty, 0, &f3, &ok);
#line 834
    Check("cs.seq: call3 ok", (sizeof("cs.seq: call3 ok") / sizeof("cs.seq: call3 ok"[0])) - 1, ok);
#line 835
    ok = RpcServer_ServeOnce(&srv);
#line 836
    ok = RpcClient_OnReadable(&cli);
#line 837
    PumpSched(sched);
#line 839
    st = ((uint32_t)(Promise_GetFate(f1, &fate)));
#line 840
    Check("cs.seq: f1 fulfilled", (sizeof("cs.seq: f1 fulfilled") / sizeof("cs.seq: f1 fulfilled"[0])) - 1, (fate == Promise_Fate_Fulfilled));
#line 841
    st = ((uint32_t)(Promise_GetFate(f2, &fate)));
#line 842
    Check("cs.seq: f2 fulfilled", (sizeof("cs.seq: f2 fulfilled") / sizeof("cs.seq: f2 fulfilled"[0])) - 1, (fate == Promise_Fate_Fulfilled));
#line 843
    st = ((uint32_t)(Promise_GetFate(f3, &fate)));
#line 844
    Check("cs.seq: f3 fulfilled", (sizeof("cs.seq: f3 fulfilled") / sizeof("cs.seq: f3 fulfilled"[0])) - 1, (fate == Promise_Fate_Fulfilled));
#line 846
    RpcClient_FreeClient(&cli);
#line 847
    RpcServer_FreeServer(&srv);
#line 848
    st = ((uint32_t)(Scheduler_SchedulerDestroy(&sched)));
#line 849
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 854
void TestConcurrent20(void) {
    RpcTest_Pipe m2_pipe;
    RpcServer_Server srv;
    RpcClient_Client cli;
    Scheduler_Scheduler sched;
    Promise_Future futures[19 + 1];
    ByteBuf_Buf bodyBuf;
    ByteBuf_BytesView body, empty;
    int ok;
    Promise_Fate fate;
    uint32_t st;
    uint32_t i, fulfilled;
#line 868
    RpcTest_CreatePipe(&m2_pipe, 0, 0);
#line 869
    st = ((uint32_t)(Scheduler_SchedulerCreate(512, &sched)));
#line 870
    ByteBuf_Init(&bodyBuf, 64);
#line 871
    empty.base = NULL;
#line 872
    empty.len = 0;
#line 874
    RpcServer_InitServer(&srv, RpcTest_ReadB, m2_pipe, RpcTest_WriteB, m2_pipe);
#line 875
    ok = RpcServer_RegisterHandler(&srv, "Echo", (sizeof("Echo") / sizeof("Echo"[0])) - 1, 4, EchoHandler, NULL);
#line 877
    RpcClient_InitClient(&cli, RpcTest_ReadA, m2_pipe, RpcTest_WriteA, m2_pipe, sched, NULL);
#line 880
    i = 0;
#line 881
    while ((i < 20)) {
#line 882
        ByteBuf_Clear(&bodyBuf);
#line 883
        ByteBuf_AppendByte(&bodyBuf, i);
#line 884
        body = ByteBuf_AsView(&bodyBuf);
#line 885
        RpcClient_Call(&cli, "Echo", (sizeof("Echo") / sizeof("Echo"[0])) - 1, 4, body, 0, &futures[i], &ok);
#line 886
        Check("concurrent: call ok", (sizeof("concurrent: call ok") / sizeof("concurrent: call ok"[0])) - 1, ok);
#line 887
        (i++);
    }
#line 891
    i = 0;
#line 892
    while ((i < 20)) {
#line 893
        ok = RpcServer_ServeOnce(&srv);
#line 894
        (i++);
    }
#line 898
    ok = RpcClient_OnReadable(&cli);
#line 899
    PumpSched(sched);
#line 902
    fulfilled = 0;
#line 903
    i = 0;
#line 904
    while ((i < 20)) {
#line 905
        st = ((uint32_t)(Promise_GetFate(futures[i], &fate)));
#line 906
        if ((fate == Promise_Fate_Fulfilled)) {
            (fulfilled++);
        }
#line 907
        (i++);
    }
#line 909
    Check("concurrent: all 20 fulfilled", (sizeof("concurrent: all 20 fulfilled") / sizeof("concurrent: all 20 fulfilled"[0])) - 1, (fulfilled == 20));
#line 911
    RpcClient_FreeClient(&cli);
#line 912
    RpcServer_FreeServer(&srv);
#line 913
    ByteBuf_Free(&bodyBuf);
#line 914
    st = ((uint32_t)(Scheduler_SchedulerDestroy(&sched)));
#line 915
    RpcTest_DestroyPipe(&m2_pipe);
}

#line 920
void TestErrorStrings(void) {
    char s[31 + 1];
#line 923
    RpcErrors_ToString(RpcErrors_Ok, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 924
    Check("errors: Ok", (sizeof("errors: Ok") / sizeof("errors: Ok"[0])) - 1, (s[0] == 'O'));
#line 925
    RpcErrors_ToString(RpcErrors_BadRequest, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 926
    Check("errors: BadRequest", (sizeof("errors: BadRequest") / sizeof("errors: BadRequest"[0])) - 1, (s[0] == 'B'));
#line 927
    RpcErrors_ToString(RpcErrors_UnknownMethod, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 928
    Check("errors: UnknownMethod", (sizeof("errors: UnknownMethod") / sizeof("errors: UnknownMethod"[0])) - 1, (s[0] == 'U'));
#line 929
    RpcErrors_ToString(RpcErrors_Timeout, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 930
    Check("errors: Timeout", (sizeof("errors: Timeout") / sizeof("errors: Timeout"[0])) - 1, (s[0] == 'T'));
#line 931
    RpcErrors_ToString(RpcErrors_Internal, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 932
    Check("errors: Internal", (sizeof("errors: Internal") / sizeof("errors: Internal"[0])) - 1, (s[0] == 'I'));
#line 933
    RpcErrors_ToString(RpcErrors_TooLarge, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 934
    Check("errors: TooLarge", (sizeof("errors: TooLarge") / sizeof("errors: TooLarge"[0])) - 1, (s[0] == 'T'));
#line 935
    RpcErrors_ToString(RpcErrors_Closed, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 936
    Check("errors: Closed", (sizeof("errors: Closed") / sizeof("errors: Closed"[0])) - 1, (s[0] == 'C'));
#line 937
    RpcErrors_ToString(99, s, (sizeof(s) / sizeof(s[0])) - 1);
#line 938
    Check("errors: Unknown", (sizeof("errors: Unknown") / sizeof("errors: Unknown"[0])) - 1, (s[0] == 'U'));
}
int main(int _m2_argc, char **_m2_argv) {
    m2_argc = _m2_argc; m2_argv = _m2_argv;
    Promise_init();
    RpcClient_init();
#line 36
#line 942
    passed = 0;
#line 943
    failed = 0;
#line 944
    total = 0;
#line 946
    TestFrameComplete();
#line 947
    TestFrameSplitHeader();
#line 948
    TestFrameSplitPayload();
#line 949
    TestFrameTooLarge();
#line 950
    TestFrameZeroLen();
#line 951
    TestFrameClosedHeader();
#line 952
    TestWriteFrameRoundtrip();
#line 953
    TestCodecRequest();
#line 954
    TestCodecResponse();
#line 955
    TestCodecError();
#line 956
    TestCodecTruncated();
#line 957
    TestCodecBadVersion();
#line 958
    TestCodecBadType();
#line 959
    TestCodecEmptyBody();
#line 960
    TestPipeBasic();
#line 961
    TestPipePartialRead();
#line 962
    TestPipePartialWrite();
#line 963
    TestPipeClose();
#line 964
    TestPipeBidir();
#line 965
    TestServerPing();
#line 966
    TestServerUnknown();
#line 967
    TestClientServerBasic();
#line 968
    TestClientServerSequential();
#line 969
    TestConcurrent20();
#line 970
    TestErrorStrings();
#line 972
    m2_WriteLn();
#line 973
    m2_WriteString("m2rpc tests: ");
#line 974
    m2_WriteInt(passed, 0);
#line 975
    m2_WriteString(" passed, ");
#line 976
    m2_WriteInt(failed, 0);
#line 977
    m2_WriteString(" failed out of ");
#line 978
    m2_WriteInt(total, 0);
#line 979
    m2_WriteLn();
#line 981
    if ((failed > 0)) {
#line 982
        m2_WriteString("SOME TESTS FAILED");
        m2_WriteLn();
    } else {
#line 984
        m2_WriteString("ALL TESTS PASSED");
        m2_WriteLn();
    }
    return 0;
}
