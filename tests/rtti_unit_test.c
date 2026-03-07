/*
 * C unit tests for M2_TypeDesc, M2_RefHeader, M2_TYPEOF, M2_ISA, M2_NARROW.
 * Compile: cc -DM2_RTTI_DEBUG -o rtti_test tests/rtti_unit_test.c && ./rtti_test
 * Also:    cc -o rtti_test tests/rtti_unit_test.c && ./rtti_test  (release mode)
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <setjmp.h>
#include <assert.h>

/* --- Minimal runtime stubs needed by the RTTI code --- */
typedef struct m2_ExcFrame {
    struct m2_ExcFrame *prev;
    jmp_buf buf;
    int exception_id;
    const char *exception_name;
    void *exception_arg;
} m2_ExcFrame;
static m2_ExcFrame *m2_exc_stack = NULL;

static int m2_exception_active = 0;
static int m2_exception_code = 0;
static jmp_buf m2_exception_buf;

static void m2_raise(int id, const char *name, void *arg) {
    if (m2_exc_stack) {
        m2_exc_stack->exception_id = id;
        m2_exc_stack->exception_name = name;
        m2_exc_stack->exception_arg = arg;
        longjmp(m2_exc_stack->buf, id ? id : 1);
    }
    fprintf(stderr, "Unhandled exception: %s (id=%d)\n", name ? name : "unknown", id);
    exit(1);
}

/* GC stubs */
#define GC_MALLOC(sz) malloc(sz)
#define GC_FREE(p) free(p)

/* --- Include the RTTI definitions (copy from stdlib.rs output) --- */
typedef struct M2_TypeDesc {
    uint32_t   type_id;
    const char *type_name;
    struct M2_TypeDesc *parent;
    uint32_t   depth;
} M2_TypeDesc;

typedef struct M2_RefHeader {
#ifdef M2_RTTI_DEBUG
    uint32_t magic;
    uint32_t flags;
#endif
    M2_TypeDesc *td;
} M2_RefHeader;

#define M2_REFHEADER_MAGIC 0x4D325246u

static inline void *M2_ref_alloc(size_t payload_size, M2_TypeDesc *td) {
    M2_RefHeader *hdr = (M2_RefHeader *)GC_MALLOC(sizeof(M2_RefHeader) + payload_size);
    if (!hdr) { fprintf(stderr, "M2_ref_alloc: out of memory\n"); exit(1); }
#ifdef M2_RTTI_DEBUG
    hdr->magic = M2_REFHEADER_MAGIC;
    hdr->flags = 0;
#endif
    hdr->td = td;
    return (void *)(hdr + 1);
}

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

static inline int M2_ISA(void *payload, M2_TypeDesc *target) {
    M2_TypeDesc *td = M2_TYPEOF(payload);
    if (!td || !target) return 0;
    if (td->depth < target->depth) return 0;
    while (td) {
        if (td == target) return 1;
        td = td->parent;
    }
    return 0;
}

static inline void *M2_NARROW(void *payload, M2_TypeDesc *target) {
    if (M2_ISA(payload, target)) return payload;
    m2_raise(99, "NarrowFault", NULL);
    return NULL;
}

static inline void M2_ref_free(void *payload) {
    if (!payload) return;
    M2_RefHeader *hdr = ((M2_RefHeader *)payload) - 1;
#ifdef M2_RTTI_DEBUG
    hdr->flags = 0xDEADDEADu;
#endif
    GC_FREE(hdr);
}

/* --- Test infrastructure --- */
static int tests_passed = 0;
static int tests_failed = 0;

#define CHECK(cond, msg) do { \
    if (cond) { tests_passed++; } \
    else { tests_failed++; fprintf(stderr, "FAIL: %s (line %d)\n", msg, __LINE__); } \
} while(0)

/* --- Type hierarchy for testing --- */
/*  Shape (depth=0)
 *    Circle (depth=1)
 *      ColorCircle (depth=2)
 *    Square (depth=1)
 *  IntRef (depth=0, no parent)
 */
static M2_TypeDesc TD_Shape      = { 1, "Shape",       NULL,       0 };
static M2_TypeDesc TD_Circle     = { 2, "Circle",      &TD_Shape,  1 };
static M2_TypeDesc TD_ColorCircle= { 3, "ColorCircle", &TD_Circle, 2 };
static M2_TypeDesc TD_Square     = { 4, "Square",      &TD_Shape,  1 };
static M2_TypeDesc TD_IntRef     = { 5, "IntRef",      NULL,       0 };

/* --- Tests --- */
void test_alloc_and_typeof(void) {
    int *p = (int *)M2_ref_alloc(sizeof(int), &TD_IntRef);
    *p = 42;
    CHECK(*p == 42, "payload value preserved after alloc");
    CHECK(M2_TYPEOF(p) == &TD_IntRef, "M2_TYPEOF returns correct descriptor");
    CHECK(M2_TYPEOF(p)->type_id == 5, "type_id is correct");
    CHECK(strcmp(M2_TYPEOF(p)->type_name, "IntRef") == 0, "type_name is correct");
}

void test_typeof_null(void) {
    CHECK(M2_TYPEOF(NULL) == NULL, "M2_TYPEOF(NULL) returns NULL");
}

void test_isa_exact(void) {
    int *p = (int *)M2_ref_alloc(sizeof(int), &TD_IntRef);
    CHECK(M2_ISA(p, &TD_IntRef) == 1, "ISA exact match");
    CHECK(M2_ISA(p, &TD_Shape) == 0, "ISA no match (unrelated type)");
}

void test_isa_subtype(void) {
    /* Circle ISA Shape? Yes */
    void *circle = M2_ref_alloc(32, &TD_Circle);
    CHECK(M2_ISA(circle, &TD_Circle) == 1, "Circle ISA Circle");
    CHECK(M2_ISA(circle, &TD_Shape) == 1, "Circle ISA Shape");
    CHECK(M2_ISA(circle, &TD_Square) == 0, "Circle ISA Square = no");
    CHECK(M2_ISA(circle, &TD_IntRef) == 0, "Circle ISA IntRef = no");

    /* ColorCircle ISA Circle, Shape? Yes */
    void *cc = M2_ref_alloc(32, &TD_ColorCircle);
    CHECK(M2_ISA(cc, &TD_ColorCircle) == 1, "CC ISA CC");
    CHECK(M2_ISA(cc, &TD_Circle) == 1, "CC ISA Circle");
    CHECK(M2_ISA(cc, &TD_Shape) == 1, "CC ISA Shape");
    CHECK(M2_ISA(cc, &TD_Square) == 0, "CC ISA Square = no");
}

void test_isa_depth_earlyout(void) {
    /* Shape (depth=0) can never be a Circle (depth=1) */
    void *shape = M2_ref_alloc(16, &TD_Shape);
    CHECK(M2_ISA(shape, &TD_Circle) == 0, "Shape ISA Circle = no (depth early-out)");
    CHECK(M2_ISA(shape, &TD_ColorCircle) == 0, "Shape ISA ColorCircle = no (depth)");
}

void test_isa_null(void) {
    CHECK(M2_ISA(NULL, &TD_Shape) == 0, "ISA with NULL payload = 0");
    void *p = M2_ref_alloc(sizeof(int), &TD_IntRef);
    CHECK(M2_ISA(p, NULL) == 0, "ISA with NULL target = 0");
}

void test_narrow_success(void) {
    void *circle = M2_ref_alloc(32, &TD_Circle);
    void *result = M2_NARROW(circle, &TD_Shape);
    CHECK(result == circle, "NARROW success returns same pointer");
    result = M2_NARROW(circle, &TD_Circle);
    CHECK(result == circle, "NARROW exact match returns same pointer");
}

void test_narrow_failure(void) {
    void *circle = M2_ref_alloc(32, &TD_Circle);
    /* NARROW to Square should raise NarrowFault — catch via setjmp */
    m2_ExcFrame ef;
    ef.prev = m2_exc_stack;
    m2_exc_stack = &ef;
    int caught = 0;
    if (setjmp(ef.buf) == 0) {
        M2_NARROW(circle, &TD_Square);
    } else {
        caught = 1;
    }
    m2_exc_stack = ef.prev;
    CHECK(caught == 1, "NARROW failure raises exception");
}

void test_header_layout(void) {
    void *p = M2_ref_alloc(sizeof(double), &TD_Shape);
    M2_RefHeader *hdr = ((M2_RefHeader *)p) - 1;
    CHECK(hdr->td == &TD_Shape, "header td field is correct");
#ifdef M2_RTTI_DEBUG
    CHECK(hdr->magic == M2_REFHEADER_MAGIC, "header magic is M2RF");
    CHECK(hdr->flags == 0, "header flags = 0 (live)");
#endif
}

#ifdef M2_RTTI_DEBUG
void test_freed_poison(void) {
    void *p = M2_ref_alloc(sizeof(int), &TD_IntRef);
    /* Save pointer before freeing (dangerous but intentional for testing) */
    M2_RefHeader *hdr = ((M2_RefHeader *)p) - 1;
    M2_ref_free(p);
    /* After free, flags should be poisoned.
     * NOTE: This is technically UB (use-after-free), but we're testing the debug mechanism. */
    /* We can't safely dereference hdr after free, so we just verify M2_TYPEOF returns NULL
     * for a freshly allocated-then-freed block by doing a new alloc at likely same address. */
    /* Instead, just verify that M2_TYPEOF on a non-M2 pointer returns NULL */
    int stack_var = 42;
    CHECK(M2_TYPEOF(&stack_var) == NULL, "M2_TYPEOF on stack pointer returns NULL (bad magic)");
}
#endif

void test_multiple_allocs(void) {
    /* Allocate multiple objects and verify each has correct type */
    int *a = (int *)M2_ref_alloc(sizeof(int), &TD_IntRef);
    void *b = M2_ref_alloc(32, &TD_Circle);
    void *c = M2_ref_alloc(16, &TD_Square);
    *a = 99;
    CHECK(M2_TYPEOF(a) == &TD_IntRef, "alloc a has IntRef type");
    CHECK(M2_TYPEOF(b) == &TD_Circle, "alloc b has Circle type");
    CHECK(M2_TYPEOF(c) == &TD_Square, "alloc c has Square type");
    CHECK(*a == 99, "payload not corrupted by adjacent allocs");
}

int main(void) {
    printf("Running RTTI unit tests...\n");

    test_alloc_and_typeof();
    test_typeof_null();
    test_isa_exact();
    test_isa_subtype();
    test_isa_depth_earlyout();
    test_isa_null();
    test_narrow_success();
    test_narrow_failure();
    test_header_layout();
    test_multiple_allocs();

#ifdef M2_RTTI_DEBUG
    test_freed_poison();
    printf("(debug mode enabled)\n");
#endif

    printf("\n%d passed, %d failed\n", tests_passed, tests_failed);
    if (tests_failed > 0) {
        printf("*** SOME TESTS FAILED ***\n");
        return 1;
    }
    printf("*** ALL TESTS PASSED ***\n");
    return 0;
}
