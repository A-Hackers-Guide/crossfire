# Target OS matrix

What Crossfire can convert a source project *to*, and what it can build *where*.

## Convert targets

| Target | Rewrite rules | Portability shim | Build system | Packaging |
|---|---|---|---|---|
| Windows | ✅ | ✅ | CMake + MSVC / clang-cl / MinGW | `.exe` / `.msi` |
| macOS | ✅ (universal) | ✅ | CMake + AppleClang | `.dmg` / `.app` |
| Linux | ✅ | ✅ | CMake + GCC / Clang | AppImage / `.deb` / `.rpm` |
| FreeBSD | ✅ | ✅ | CMake + Clang | binary tarball |

Every convert pass writes the same shim (`crossfire/cf_platform.h`), the same `CMakeLists.txt` + `CMakePresets.json`, and the same multi-OS `.github/workflows/build.yml`. You can skip targets per project in the Configure step.

## Build where

| Host OS | Builds natively | Cross-compiles | Falls back to CI |
|---|---|---|---|
| macOS | macOS universal | Windows (MinGW / zig), Linux (zig), FreeBSD (zig) | — |
| Linux | Linux | Windows (MinGW), FreeBSD (zig) | macOS |
| Windows | Windows | — | macOS, Linux, FreeBSD |
| FreeBSD | FreeBSD | — | macOS, Windows, Linux |

Anything your host can't produce locally is still built through the GitHub Actions workflow Crossfire generates inside your converted project.

## Backends per category

| Category | Windows | macOS | Linux | FreeBSD |
|---|---|---|---|---|
| Windowing | Win32 / UWP | Cocoa / AppKit | X11 / Wayland | X11 |
| Graphics | D3D11 / D3D12 / Vulkan | Metal / Vulkan (MoltenVK) / OpenGL | Vulkan / OpenGL | Vulkan / OpenGL |
| Audio | XAudio2 / WASAPI | Core Audio | ALSA / PulseAudio / PipeWire | OSS / PulseAudio |
| Input | DirectInput / XInput | GameController | SDL / evdev | SDL / evdev |
| Filesystem | Win32 + `\\?\` long paths | POSIX | POSIX + `inotify` | POSIX + `kqueue` |
| Threading | `std::thread` / Win32 | `std::thread` / pthreads | `std::thread` / pthreads | `std::thread` / pthreads |
| Timing | QPC → `std::chrono::steady_clock` | mach_absolute_time → `std::chrono` | `clock_gettime` → `std::chrono` | `clock_gettime` → `std::chrono` |

Default choices come from your Analyze heat map; you can override every one in Configure.
