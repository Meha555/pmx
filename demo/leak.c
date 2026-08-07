#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#define SLEEP_SECONDS(seconds) Sleep((seconds) * 1000)
#else
#include <unistd.h>
#define SLEEP_SECONDS(seconds) sleep(seconds)
#endif

int main(void) {
    enum { chunk_size = 1024 * 1024 };
    unsigned long iteration = 0;

    fprintf(stderr, "leak demo started; press Ctrl+C to stop");

    for (;;) {
        void *memory = malloc(chunk_size);
        if (memory == NULL) {
            fprintf(stderr, "malloc failed after %lu iterations\n", iteration);
            return 2;
        }
        memset(memory, (int)(iteration % 255), chunk_size);

        char path[256];
#ifdef _WIN32
        snprintf(path, sizeof(path), "pmx-leak-demo-%lu.tmp", iteration);
#else
        snprintf(path, sizeof(path), "/tmp/pmx-leak-demo-%lu.tmp", iteration);
#endif
        FILE *file = fopen(path, "a+");
        if (file == NULL) {
            fprintf(stderr, "fopen(%s) failed: %s\n", path, strerror(errno));
            return 3;
        }
        fprintf(file, "iteration=%lu\n", iteration);
        fflush(file);

        fprintf(stderr, "iteration=%lu leaked_memory_mb=%lu leaked_files=%lu\n", iteration, iteration + 1, iteration + 1);
        iteration++;
        SLEEP_SECONDS(1);
    }

    return 0;
}
