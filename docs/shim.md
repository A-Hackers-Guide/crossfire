# `cf_platform.h`

Every converted project gets a header-only portability shim at `crossfire/cf_platform.h`. It's small, auditable, and standard C99 so any language that can FFI with C can call into it.

## What the shim gives you

```c
#include "crossfire/cf_platform.h"

cf_init();                      // boilerplate init for every target
cf_sleep_ms(16);                // portable Sleep / nanosleep / mach_wait_until
uint64_t ns = cf_now_ns();      // monotonic nanoseconds
cf_path_t p = cf_path_home();   // per-OS user home + per-OS slashes

cf_file_t f = cf_file_open(p, "r");
cf_file_read(f, buf, n);
cf_file_close(f);

cf_thread_t t = cf_thread_spawn(worker, &arg);
cf_thread_join(t);

cf_env_set("FOO", "bar");
char buf[256];
cf_env_get("PATH", buf, sizeof buf);
```

Every call picks the right OS API under the hood:

| Call | Windows | macOS | Linux / BSD |
|---|---|---|---|
| `cf_sleep_ms` | `Sleep` | `nanosleep` | `nanosleep` |
| `cf_now_ns` | `QueryPerformanceCounter` | `mach_absolute_time` | `clock_gettime(CLOCK_MONOTONIC)` |
| `cf_path_home` | `SHGetFolderPath` | `NSHomeDirectory` | `$HOME` |
| `cf_file_open` | `CreateFile` | `fopen` | `fopen` |
| `cf_thread_spawn` | `CreateThread` | `pthread_create` | `pthread_create` |

## Why a shim and not SDL

SDL3 is the default porting strategy for anything with a game loop, but the shim is intentionally smaller than SDL: it's the layer under SDL, for projects that don't want to pull in a UI toolkit. If you pick "SDL3 portability layer" in Configure, Crossfire still generates the shim but has SDL do the heavy lifting for input, audio and windowing.

## Opting out

If you'd rather use your own portability layer, pick **Native per OS** in Configure. Crossfire still runs the rule-based rewrites and still generates CMake/CI, but skips the shim.
