#include <stdio.h>
int test_stdio(int* p){ if (p) (*p)++; return *p; }
void show_count(int *p) { printf("%d\n", test_stdio(p)); }
