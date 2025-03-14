#include <stdio.h>
#include <stdlib.h>
#include <sys/time.h>

#define N 30

int sum(int x, int y) {
    __builtin_assume(x >= 0);
    __builtin_assume(x < N);
    __builtin_assume(y >= 0);
    __builtin_assume(y < N);
    return x + y;
}

int main(int argc, char** argv) {
    int* data = malloc(sizeof(*data) * N * N);
    if (!data) { fprintf(stderr, "Virtual memory exhausted\n");
        exit(1);
    }

    struct timeval start;
    if (gettimeofday(&start, NULL) == -1) {
        perror("gettimeofday");
        return 1;
    }
    for (int k = 0; k < N; k++) {
        for (int i = 0; i < N; i++) {
            for (int j = 0; j < N; j++) {
                int result = sum(i, j);
                if (result != i + j) {
                    fprintf(stderr, "BRUH.... %d + %d = %d\n", i, j, result);
                    exit(1);
                }
                data[i * N + j] = result;
            }
        }
    }
    struct timeval end;
    if (gettimeofday(&end, NULL) == -1) {
        perror("gettimeofday");
        return 1;
    }

    double start_seconds = (double)start.tv_sec + (double)start.tv_usec / 1e6;
    double end_seconds = (double)end.tv_sec + (double)end.tv_usec / 1e6;

    printf("%f\n", end_seconds - start_seconds);

    free(data);
}
