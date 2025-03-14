#include <stdbool.h>

int add1(int a) {
    static bool foo[10];
    return a + 1;
}
