#include <string>
int test_string(int* p){ if (p) (*p)++; return *p; }
std::string label(int* p) { return std::to_string(test_string(p)); }
