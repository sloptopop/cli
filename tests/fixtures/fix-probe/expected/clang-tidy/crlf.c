#include <stdlib.h>
#include <string.h>
char *dup(const char *s) {
    char *p = malloc(strlen(s) + 1);
    if (strcmp(s, "a")) return p;
    return 0;
}
long mul(int a, int b) { return a * b; }
