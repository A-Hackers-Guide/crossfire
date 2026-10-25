//! Generated portability shims (embedded verbatim into the converted project).

/// C/C++ portability header.
pub const CF_PLATFORM_H: &str = include_str!("shim/cf_platform.h");
/// C implementation of the helpers declared in cf_platform.h.
pub const CF_PLATFORM_C: &str = include_str!("shim/cf_platform.c");
/// Python helpers (crossfire/cf_platform.py).
pub const CF_PLATFORM_PY: &str = include_str!("shim/cf_platform.py");
/// Python winreg emulation (crossfire/cf_winreg.py).
pub const CF_WINREG_PY: &str = include_str!("shim/cf_winreg.py");
/// .NET helpers (Crossfire/CfPlatform.cs).
pub const CF_PLATFORM_CS: &str = include_str!("shim/CfPlatform.cs");

/// Python msvcrt emulation (crossfire/cf_msvcrt.py).
pub const CF_MSVCRT_PY: &str = r#""""crossfire.cf_msvcrt — portable subset of `msvcrt` (getch/getwch/kbhit/putch)."""
import sys

if sys.platform == "win32":
    from msvcrt import *  # noqa: F401,F403
else:
    from . import cf_platform

    def getwch():
        return cf_platform.getch()

    def getch():
        return cf_platform.getch().encode("utf-8", "replace")

    def getche():
        c = getch()
        sys.stdout.write(c.decode("utf-8", "replace"))
        sys.stdout.flush()
        return c

    def kbhit():
        return cf_platform.kbhit()

    def putch(ch):
        sys.stdout.write(ch.decode() if isinstance(ch, bytes) else ch)
        sys.stdout.flush()

    putwch = putch
"#;

pub const CF_INIT_PY: &str = "\"\"\"Crossfire portability helpers (generated).\"\"\"\n";
