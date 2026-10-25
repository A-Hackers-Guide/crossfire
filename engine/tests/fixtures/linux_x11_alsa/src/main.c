/* Bouncer: a tiny software-rendered toy for Linux (X11 + ALSA). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <limits.h>
#include "platform.h"

#ifdef __linux__
#include <sys/sysinfo.h>
#endif

#define W 320
#define H 200

static double now_seconds(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec / 1e9;
}

static void data_dir(char *out, size_t size)
{
    char exe[PATH_MAX];
    ssize_t n = readlink("/proc/self/exe", exe, sizeof exe - 1);
    if (n < 0) n = 0;
    exe[n] = '\0';
    char *slash = strrchr(exe, '/');
    if (slash) *slash = '\0';
    snprintf(out, size, "%s/data", exe);
}

int main(void)
{
    static unsigned int pixels[W * H];
    char dir[PATH_MAX];
    data_dir(dir, sizeof dir);
    printf("data dir: %s\n", dir);
#ifdef __linux__
    printf("cpus: %d\n", get_nprocs());
#endif
    if (!platform_window_open(W, H, "Bouncer")) {
        fprintf(stderr, "no window\n");
        return 1;
    }
    platform_audio_open(44100);
    double x = 10, y = 10, vx = 90, vy = 70, last = now_seconds();
    int key = 0, frames = 0;
    while (platform_window_poll(&key) && key != 'q' && frames < 600) {
        double t = now_seconds(), dt = t - last;
        last = t;
        x += vx * dt; y += vy * dt;
        if (x < 0 || x > W - 8) { vx = -vx; platform_audio_beep(440.0f, 0.05f); }
        if (y < 0 || y > H - 8) { vy = -vy; platform_audio_beep(660.0f, 0.05f); }
        memset(pixels, 0, sizeof pixels);
        for (int j = 0; j < 8; j++)
            for (int i = 0; i < 8; i++) {
                int px = (int)x + i, py = (int)y + j;
                if (px >= 0 && px < W && py >= 0 && py < H) pixels[py * W + px] = 0xffcc33;
            }
        platform_window_present(pixels, W, H);
        usleep(16000);
        frames++;
    }
    platform_audio_close();
    platform_window_close();
    return 0;
}
