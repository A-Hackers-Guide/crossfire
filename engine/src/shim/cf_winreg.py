"""crossfire.cf_winreg — drop-in subset of the `winreg` module.

On Windows this simply re-exports the real winreg. Elsewhere, keys and values
are stored in a JSON file under the user config directory, so code written
against the registry keeps working:

    from crossfire import cf_winreg as winreg
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, r"Software\\MyApp") as k:
        winreg.SetValueEx(k, "Volume", 0, winreg.REG_DWORD, 7)
"""
import sys

if sys.platform == "win32":
    from winreg import *  # noqa: F401,F403
else:
    import json
    import os
    import threading

    from . import cf_platform

    HKEY_CLASSES_ROOT = 0x80000000
    HKEY_CURRENT_USER = 0x80000001
    HKEY_LOCAL_MACHINE = 0x80000002
    HKEY_USERS = 0x80000003
    HKEY_CURRENT_CONFIG = 0x80000005
    _HIVES = {
        HKEY_CLASSES_ROOT: "HKCR", HKEY_CURRENT_USER: "HKCU", HKEY_LOCAL_MACHINE: "HKLM",
        HKEY_USERS: "HKU", HKEY_CURRENT_CONFIG: "HKCC",
    }
    KEY_QUERY_VALUE, KEY_SET_VALUE, KEY_CREATE_SUB_KEY = 0x1, 0x2, 0x4
    KEY_ENUMERATE_SUB_KEYS, KEY_READ, KEY_WRITE, KEY_ALL_ACCESS = 0x8, 0x20019, 0x20006, 0xF003F
    KEY_WOW64_64KEY, KEY_WOW64_32KEY = 0x100, 0x200
    REG_NONE, REG_SZ, REG_EXPAND_SZ, REG_BINARY, REG_DWORD = 0, 1, 2, 3, 4
    REG_MULTI_SZ, REG_QWORD = 7, 11

    _lock = threading.RLock()
    _path = os.environ.get("CF_REGISTRY_FILE") or os.path.join(
        cf_platform.user_config_dir(), "crossfire-registry.json")

    def _load():
        try:
            with open(_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (OSError, ValueError):
            return {}

    def _save(data):
        os.makedirs(os.path.dirname(_path), exist_ok=True)
        tmp = _path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, sort_keys=True)
        os.replace(tmp, _path)

    class HKEYType:
        """Handle to an emulated key; usable as a context manager."""

        def __init__(self, path):
            self.path = path
            self.handle = id(self)

        def Close(self):
            pass

        def Detach(self):
            return self.handle

        def __enter__(self):
            return self

        def __exit__(self, *exc):
            return False

        def __int__(self):
            return self.handle

    PyHKEY = HKEYType

    def _norm(key, sub_key):
        base = key.path if isinstance(key, HKEYType) else _HIVES.get(key, str(key))
        parts = [p for p in (sub_key or "").replace("/", "\\").split("\\") if p]
        return "\\".join([base] + parts)

    def _keys(data):
        return data.setdefault("keys", {})

    def CreateKey(key, sub_key):
        path = _norm(key, sub_key)
        with _lock:
            data = _load()
            keys = _keys(data)
            # Create every ancestor too, like the real registry.
            parts = path.split("\\")
            for i in range(1, len(parts) + 1):
                keys.setdefault("\\".join(parts[:i]), {})
            _save(data)
        return HKEYType(path)

    def CreateKeyEx(key, sub_key, reserved=0, access=KEY_WRITE):
        return CreateKey(key, sub_key)

    def OpenKey(key, sub_key, reserved=0, access=KEY_READ):
        path = _norm(key, sub_key)
        with _lock:
            if path not in _keys(_load()) and path not in _HIVES.values():
                raise FileNotFoundError(2, "The system cannot find the file specified", path)
        return HKEYType(path)

    OpenKeyEx = OpenKey

    def CloseKey(hkey):
        pass

    def QueryValueEx(key, value_name):
        path = _norm(key, "")
        with _lock:
            values = _keys(_load()).get(path)
        if values is None or (value_name or "") not in values:
            raise FileNotFoundError(2, "The system cannot find the file specified", value_name)
        v = values[value_name or ""]
        return v["value"], v["type"]

    def QueryValue(key, sub_key):
        return QueryValueEx(OpenKey(key, sub_key), "")[0]

    def SetValueEx(key, value_name, reserved, type, value):
        path = _norm(key, "")
        if isinstance(value, (bytes, bytearray)):
            value = list(value)
        with _lock:
            data = _load()
            _keys(data).setdefault(path, {})[value_name or ""] = {"type": type, "value": value}
            _save(data)

    def SetValue(key, sub_key, type, value):
        SetValueEx(CreateKey(key, sub_key), "", 0, type, value)

    def DeleteValue(key, value):
        path = _norm(key, "")
        with _lock:
            data = _load()
            vals = _keys(data).get(path, {})
            if value not in vals:
                raise FileNotFoundError(2, "The system cannot find the file specified", value)
            del vals[value]
            _save(data)

    def DeleteKey(key, sub_key):
        path = _norm(key, sub_key)
        with _lock:
            data = _load()
            keys = _keys(data)
            if path not in keys:
                raise FileNotFoundError(2, "The system cannot find the file specified", path)
            if any(k.startswith(path + "\\") for k in keys):
                raise PermissionError(5, "Access is denied (key has subkeys)", path)
            del keys[path]
            _save(data)

    def EnumKey(key, index):
        path = _norm(key, "")
        with _lock:
            children = sorted({k[len(path) + 1:].split("\\")[0] for k in _keys(_load()) if k.startswith(path + "\\")})
        if index >= len(children):
            raise OSError(259, "No more data is available")
        return children[index]

    def EnumValue(key, index):
        path = _norm(key, "")
        with _lock:
            vals = sorted(_keys(_load()).get(path, {}).items())
        if index >= len(vals):
            raise OSError(259, "No more data is available")
        name, v = vals[index]
        return name, v["value"], v["type"]

    def QueryInfoKey(key):
        path = _norm(key, "")
        with _lock:
            keys = _keys(_load())
        subkeys = {k[len(path) + 1:].split("\\")[0] for k in keys if k.startswith(path + "\\")}
        return len(subkeys), len(keys.get(path, {})), 0

    def FlushKey(key):
        pass

    def ExpandEnvironmentStrings(s):
        return os.path.expandvars(s.replace("%", "$")) if "%" in s else s
