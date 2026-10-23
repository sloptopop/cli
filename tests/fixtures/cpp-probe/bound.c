int sum_orders(const int *amounts) {
    int copy[9];
    int total = 0;
    for (int i = 0; i <= 9; i++) {
        copy[i] = amounts[i];
        total += copy[i];
    }
    return total;
}
