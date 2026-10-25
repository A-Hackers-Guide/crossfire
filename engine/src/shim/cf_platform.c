/*
 * crossfire/cf_platform.c — implementation of the Crossfire portability layer.
 * Builds as C99/C11 on Windows (MSVC, MinGW, clang-cl), macOS, Linux and FreeBSD.
 */
#if defined(__linux__) && !defined(_GNU_SOURCE)
#  define _GNU_SOURCE 1
#endif

#include "cf_platform.h"

#if defined(CF_USE_SDL3)
#  include <SDL3/SDL.h>
#endif

#if defined(_WIN32)
#  include <shellapi.h>
#  include <conio.h>
#  if defined(_MSC_VER)
#    pragma comment(lib, "shell32.lib")
#    pragma comment(lib, "ws2_32.lib")
#  endif
#else
#  include <dlfcn.h>
#  include <termios.h>
#  include <fcntl.h>
#  include <spawn.h>
#  include <sys/select.h>
#  include <sys/wait.h>
#  include <pwd.h>
#  include <signal.h>
extern char **environ;
#endif

#if defined(__APPLE__)
#  include <mach-o/dyld.h>
#  include <mach/mach_time.h>
#endif

#if defined(__FreeBSD__) || defined(__DragonFly__)
#  include <sys/sysctl.h>
#  include <pthread_np.h>
#endif

/* Small fixed buffers for the path helpers (computed once, then cached). */
#define CF_PATH_BUF 4096
static char cf_buf_data[CF_PATH_BUF], cf_buf_home[CF_PATH_BUF], cf_buf_tmp[CF_PATH_BUF];
static char cf_buf_exe[CF_PATH_BUF], cf_buf_res[CF_PATH_BUF];

/* ------------------------------------------------------------------ timing */
void cf_sleep_ms(uint32_t ms) {
#if defined(_WIN32)
    Sleep(ms);
#else
    struct timespec ts;
    ts.tv_sec = (time_t)(ms / 1000u);
    ts.tv_nsec = (long)(ms % 1000u) * 1000000L;
    while (nanosleep(&ts, &ts) == -1 && errno == EINTR) {}
#endif
}

void cf_sleep_us(uint64_t us) {
#if defined(_WIN32)
    Sleep((DWORD)((us + 999u) / 1000u));
#else
    struct timespec ts;
    ts.tv_sec = (time_t)(us / 1000000u);
    ts.tv_nsec = (long)(us % 1000000u) * 1000L;
    while (nanosleep(&ts, &ts) == -1 && errno == EINTR) {}
#endif
}

uint64_t cf_ticks_ns(void) {
#if defined(_WIN32)
    static LARGE_INTEGER freq;
    LARGE_INTEGER now;
    if (!freq.QuadPart) QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&now);
    return (uint64_t)((double)now.QuadPart * 1e9 / (double)freq.QuadPart);
#elif defined(__APPLE__)
    static mach_timebase_info_data_t tb;
    if (!tb.denom) mach_timebase_info(&tb);
    return mach_absolute_time() * tb.numer / tb.denom;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
#endif
}

uint64_t cf_ticks_ms(void) { return cf_ticks_ns() / 1000000ull; }

/* ----------------------------------------------------------------- strings */
int cf_stricmp(const char *a, const char *b) {
#if defined(_WIN32)
    return _stricmp(a, b);
#else
    return strcasecmp(a, b);
#endif
}

int cf_strnicmp(const char *a, const char *b, size_t n) {
#if defined(_WIN32)
    return _strnicmp(a, b, n);
#else
    return strncasecmp(a, b, n);
#endif
}

size_t cf_strlcpy(char *dst, const char *src, size_t size) {
    size_t len = strlen(src);
    if (size) {
        size_t n = len < size - 1 ? len : size - 1;
        memcpy(dst, src, n);
        dst[n] = '\0';
    }
    return len;
}

/* ------------------------------------------------------------------- paths */
static const char *cf_env(const char *name) {
    const char *v = getenv(name);
    return (v && *v) ? v : NULL;
}

const char *cf_home_dir(void) {
    if (cf_buf_home[0]) return cf_buf_home;
#if defined(_WIN32)
    const char *h = cf_env("USERPROFILE");
    cf_strlcpy(cf_buf_home, h ? h : "C:/", CF_PATH_BUF);
#else
    const char *h = cf_env("HOME");
    if (!h) {
        struct passwd *pw = getpwuid(getuid());
        h = pw ? pw->pw_dir : "/tmp";
    }
    cf_strlcpy(cf_buf_home, h, CF_PATH_BUF);
#endif
    return cf_buf_home;
}

const char *cf_user_data_dir(void) {
    if (cf_buf_data[0]) return cf_buf_data;
#if defined(_WIN32)
    const char *a = cf_env("APPDATA");
    snprintf(cf_buf_data, CF_PATH_BUF, "%s", a ? a : cf_home_dir());
#elif defined(__APPLE__)
    snprintf(cf_buf_data, CF_PATH_BUF, "%s/Library/Application Support", cf_home_dir());
#else
    const char *x = cf_env("XDG_DATA_HOME");
    if (x) snprintf(cf_buf_data, CF_PATH_BUF, "%s", x);
    else snprintf(cf_buf_data, CF_PATH_BUF, "%s/.local/share", cf_home_dir());
#endif
    return cf_buf_data;
}

const char *cf_temp_dir(void) {
    if (cf_buf_tmp[0]) return cf_buf_tmp;
#if defined(_WIN32)
    const char *t = cf_env("TEMP");
    if (!t) t = cf_env("TMP");
    cf_strlcpy(cf_buf_tmp, t ? t : "C:/Windows/Temp", CF_PATH_BUF);
#else
    const char *t = cf_env("TMPDIR");
    cf_strlcpy(cf_buf_tmp, t ? t : "/tmp", CF_PATH_BUF);
    size_t n = strlen(cf_buf_tmp);
    if (n > 1 && cf_buf_tmp[n - 1] == '/') cf_buf_tmp[n - 1] = '\0';
#endif
    return cf_buf_tmp;
}

const char *cf_executable_path(void) {
    if (cf_buf_exe[0]) return cf_buf_exe;
#if defined(_WIN32)
    DWORD n = GetModuleFileNameA(NULL, cf_buf_exe, CF_PATH_BUF);
    if (n == 0 || n >= CF_PATH_BUF) cf_buf_exe[0] = '\0';
#elif defined(__APPLE__)
    char tmp[CF_PATH_BUF];
    uint32_t size = sizeof tmp;
    if (_NSGetExecutablePath(tmp, &size) == 0) {
        if (!realpath(tmp, cf_buf_exe)) cf_strlcpy(cf_buf_exe, tmp, CF_PATH_BUF);
    }
#elif defined(__FreeBSD__) || defined(__DragonFly__)
    int mib[4] = { CTL_KERN, KERN_PROC, KERN_PROC_PATHNAME, -1 };
    size_t size = CF_PATH_BUF;
    if (sysctl(mib, 4, cf_buf_exe, &size, NULL, 0) != 0) cf_buf_exe[0] = '\0';
#else
    ssize_t n = readlink("/proc/self/exe", cf_buf_exe, CF_PATH_BUF - 1);
    cf_buf_exe[n > 0 ? n : 0] = '\0';
#endif
    return cf_buf_exe;
}

long cf_executable_path_into(char *buf, size_t size) {
    const char *p = cf_executable_path();
    size_t n = strlen(p);
    if (!n || !buf) return -1;
    if (n > size) n = size;
    memcpy(buf, p, n);
    return (long)n;
}

const char *cf_resource_dir(void) {
    if (cf_buf_res[0]) return cf_buf_res;
    cf_strlcpy(cf_buf_res, cf_executable_path(), CF_PATH_BUF);
#if defined(__APPLE__)
    char *macos = strstr(cf_buf_res, ".app/Contents/MacOS/");
    if (macos) {
        strcpy(macos, ".app/Contents/Resources");
        return cf_buf_res;
    }
#endif
    {
        char *slash = strrchr(cf_buf_res, '/');
#if defined(_WIN32)
        char *bs = strrchr(cf_buf_res, '\\');
        if (!slash || (bs && bs > slash)) slash = bs;
#endif
        if (slash) *slash = '\0';
    }
    return cf_buf_res;
}

/* ----------------------------------------------------------------- shell */
int cf_open_url(const char *url) {
#if defined(CF_USE_SDL3)
    return SDL_OpenURL(url) ? 0 : -1;
#elif defined(_WIN32)
    return (intptr_t)ShellExecuteA(NULL, "open", url, NULL, NULL, SW_SHOWNORMAL) > 32 ? 0 : -1;
#else
#  if defined(__APPLE__)
    const char *tool = "open";
#  else
    const char *tool = "xdg-open";
#  endif
    char *argv[3];
    pid_t pid;
    argv[0] = (char *)tool;
    argv[1] = (char *)url;
    argv[2] = NULL;
    if (posix_spawnp(&pid, tool, NULL, NULL, argv, environ) != 0) return -1;
    return 0;
#endif
}

void cf_message_box(const char *title, const char *message) {
#if defined(CF_USE_SDL3)
    if (SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_INFORMATION, title, message, NULL)) return;
#elif defined(_WIN32)
    MessageBoxA(NULL, message, title, MB_OK);
    return;
#endif
    fprintf(stderr, "[%s] %s\n", title ? title : "", message ? message : "");
}

void cf_debug_log(const char *message) {
#if defined(_WIN32)
    OutputDebugStringA(message);
#endif
    fputs(message ? message : "", stderr);
}

/* -------------------------------------------------------- dynamic libraries */
void *cf_dlopen(const char *path) {
#if defined(_WIN32)
    return (void *)LoadLibraryA(path);
#else
    return dlopen(path, RTLD_NOW | RTLD_LOCAL);
#endif
}

void *cf_dlsym(void *lib, const char *name) {
#if defined(_WIN32)
    return (void *)GetProcAddress((HMODULE)lib, name);
#else
    return dlsym(lib, name);
#endif
}

void cf_dlclose(void *lib) {
    if (!lib) return;
#if defined(_WIN32)
    FreeLibrary((HMODULE)lib);
#else
    dlclose(lib);
#endif
}

/* ---------------------------------------------------------------- memory */
void *cf_aligned_alloc(size_t alignment, size_t size) {
#if defined(_WIN32)
    return _aligned_malloc(size, alignment);
#else
    void *p = NULL;
    if (alignment < sizeof(void *)) alignment = sizeof(void *);
    return posix_memalign(&p, alignment, size) == 0 ? p : NULL;
#endif
}

void cf_aligned_free(void *p) {
#if defined(_WIN32)
    _aligned_free(p);
#else
    free(p);
#endif
}

/* --------------------------------------------------------------- console */
int cf_getch(void) {
#if defined(_WIN32)
    return _getch();
#else
    struct termios oldt, raw;
    int c;
    if (tcgetattr(STDIN_FILENO, &oldt) != 0) return getchar();
    raw = oldt;
    raw.c_lflag &= ~(tcflag_t)(ICANON | ECHO);
    tcsetattr(STDIN_FILENO, TCSANOW, &raw);
    c = getchar();
    tcsetattr(STDIN_FILENO, TCSANOW, &oldt);
    return c;
#endif
}

int cf_kbhit(void) {
#if defined(_WIN32)
    return _kbhit();
#else
    struct timeval tv = { 0, 0 };
    fd_set fds;
    FD_ZERO(&fds);
    FD_SET(STDIN_FILENO, &fds);
    return select(STDIN_FILENO + 1, &fds, NULL, NULL, &tv) > 0;
#endif
}

/* --------------------------------------------------------------- threads */
void cf_set_thread_name(const char *name) {
#if defined(__APPLE__)
    pthread_setname_np(name);
#elif defined(__FreeBSD__) || defined(__DragonFly__)
    pthread_set_name_np(pthread_self(), name);
#elif defined(__linux__)
    char shortname[16];
    cf_strlcpy(shortname, name, sizeof shortname);
    pthread_setname_np(pthread_self(), shortname);
#else
    (void)name;
#endif
}

/* ------------------------------------------------------------- networking */
int cf_net_init(void) {
#if defined(_WIN32)
    WSADATA d;
    return WSAStartup(MAKEWORD(2, 2), &d);
#else
    signal(SIGPIPE, SIG_IGN);
    return 0;
#endif
}

int cf_closesocket(intptr_t s) {
#if defined(_WIN32)
    return closesocket((SOCKET)s);
#else
    return close((int)s);
#endif
}

/* ----------------------------------------------------------------- stubs */
void cf_stub_warn(const char *function, const char *file) {
    static int warned = 0;
    if (warned++ < 20)
        fprintf(stderr, "crossfire: %s (%s) is not implemented on this OS yet\n", function, file);
}
