#include <stdio.h>
#include <stdlib.h>
#include <sys/time.h>

#define N 1000

int add1(int a) {
    __builtin_assume(a >= 0);
    __builtin_assume(a < N);
    return a + 1;
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
            int result = add1(j);
            if (result != j + 1) {
                fprintf(stderr, "BRUH.... you got %d + 1 = %d\n", j, result);
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
