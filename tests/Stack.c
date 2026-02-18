/* Modula-2 Runtime Support */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdint.h>
#include <ctype.h>
#include <limits.h>
#include <float.h>

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

/* Strings module */
static void m2_Strings_Assign(const char *src, char *dst) { strcpy(dst, src); }
static void m2_Strings_Insert(const char *sub, char *dst, uint32_t pos) {
    size_t slen = strlen(sub), dlen = strlen(dst);
    if (pos > dlen) pos = (uint32_t)dlen;
    memmove(dst + pos + slen, dst + pos, dlen - pos + 1);
    memcpy(dst + pos, sub, slen);
}
static void m2_Strings_Delete(char *s, uint32_t pos, uint32_t len) {
    size_t slen = strlen(s);
    if (pos >= slen) return;
    if (pos + len > slen) len = (uint32_t)(slen - pos);
    memmove(s + pos, s + pos + len, slen - pos - len + 1);
}
static uint32_t m2_Strings_Pos(const char *sub, const char *s) {
    const char *p = strstr(s, sub);
    return p ? (uint32_t)(p - s) : UINT32_MAX;
}
static uint32_t m2_Strings_Length(const char *s) { return (uint32_t)strlen(s); }
static void m2_Strings_Copy(const char *src, uint32_t pos, uint32_t len, char *dst) {
    size_t slen = strlen(src);
    if (pos >= slen) { dst[0] = '\0'; return; }
    if (pos + len > slen) len = (uint32_t)(slen - pos);
    memcpy(dst, src + pos, len);
    dst[len] = '\0';
}
static void m2_Strings_Concat(const char *s1, const char *s2, char *dst) {
    strcpy(dst, s1); strcat(dst, s2);
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

/* Implementation Module Stack */

typedef struct Node Node;
typedef Node *NodePtr;

struct Node {
    int32_t value;
    NodePtr next;
};

typedef NodePtr Stack;

void Create(Stack *s);
void Push(Stack *s, int32_t val);
void Pop(Stack *s, int32_t *val);
int IsEmpty(Stack s);
void Destroy(Stack *s);


void Create(Stack *s) {
    (*s) = NULL;
}

void Push(Stack *s, int32_t val) {
    NodePtr n;
    n = malloc(sizeof(*n));
    n->value = val;
    n->next = (*s);
    (*s) = n;
}

void Pop(Stack *s, int32_t *val) {
    NodePtr tmp;
    if (((*s) != NULL)) {
        (*val) = (*s)->value;
        tmp = (*s);
        (*s) = (*s)->next;
        free(tmp);
    }
}

int IsEmpty(Stack s) {
    return (s == NULL);
}

void Destroy(Stack *s) {
    NodePtr tmp;
    while (((*s) != NULL)) {
        tmp = (*s);
        (*s) = (*s)->next;
        free(tmp);
    }
}
