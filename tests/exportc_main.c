#include <stdio.h>
#include <stdint.h>

extern int32_t get_value(void);

int main(void) {
    int32_t v = get_value();
    if (v == 42) {
        printf("OK: get_value() returned %d\n", v);
        return 0;
    } else {
        printf("FAIL: get_value() returned %d, expected 42\n", v);
        return 1;
    }
}
