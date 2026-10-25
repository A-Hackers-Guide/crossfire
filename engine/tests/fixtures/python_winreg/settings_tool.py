"""Tiny settings manager for a Windows desktop tool."""
import os
import sys
import winreg
import msvcrt

KEY_PATH = r"Software\SettingsTool"
LOG_FILE = r"logs\settings.log"


def load(name, default=None):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, KEY_PATH) as key:
            value, _ = winreg.QueryValueEx(key, name)
            return value
    except FileNotFoundError:
        return default


def save(name, value):
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, KEY_PATH) as key:
        winreg.SetValueEx(key, name, 0, winreg.REG_SZ, str(value))


def export_dir():
    return os.path.join(os.environ['APPDATA'], "SettingsTool")


def main():
    os.system('cls')
    print("Theme:", load("theme", "light"))
    print("Press t to toggle theme, o to open the export folder, q to quit")
    while True:
        ch = msvcrt.getch()
        if ch == b"t":
            save("theme", "dark" if load("theme", "light") == "light" else "light")
            print("Theme:", load("theme"))
        elif ch == b"o":
            os.makedirs(export_dir(), exist_ok=True)
            os.startfile(export_dir())
        elif ch == b"q":
            break


if __name__ == "__main__":
    main()
