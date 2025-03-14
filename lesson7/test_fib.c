#include <stdio.h>
#include <stdlib.h>
#include <sys/time.h>

#define N 30

int fib(int a) {
    __builtin_assume(a >= 0);
    __builtin_assume(a < N);
    if (a == 0) {
        return 0;
    } else if (a == 1) {
        return 1;
    } else {
        return fib(a - 1) + fib(a - 2);
    }
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
    for (int i = 0; i < N; i++) {
        for (int j = 0; j < N; j++) {
            int result = fib(j);
            if (j >= 2 && result != data[i * N + j - 1] + data[i * N + j - 2]) {
                fprintf(stderr, "BRUH.... you got %dth fib was %d\n", j, result);
                exit(1);
            }
            data[i * N + j] = result;
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

    for (int i = 0; i < N; i++) {
        for (int j = 0; j < N; j++) {
            printf("%d\r", data[i * N + j]);
        }
    }

    free(data);
}
