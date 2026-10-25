using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using Microsoft.Win32;

namespace Notes
{
    public class MainForm : Form
    {
        [DllImport("user32.dll")]
        private static extern bool MessageBeep(uint type);

        private readonly TextBox _text = new TextBox { Multiline = true, Dock = DockStyle.Fill };
        private readonly string _file = Path.Combine(Environment.GetEnvironmentVariable("APPDATA") ?? ".", @"Notes\notes.txt");

        public MainForm()
        {
            Text = "Notes";
            Controls.Add(_text);
            var save = new Button { Text = "Save", Dock = DockStyle.Bottom };
            save.Click += (s, e) => Save();
            Controls.Add(save);
            var open = new Button { Text = "Open folder", Dock = DockStyle.Bottom };
            open.Click += (s, e) => Process.Start("explorer.exe", Path.GetDirectoryName(_file)!);
            Controls.Add(open);
            using var key = Registry.CurrentUser.CreateSubKey(@"Software\Notes");
            Width = (int)(key.GetValue("Width") ?? 640);
            if (File.Exists(_file)) _text.Text = File.ReadAllText(_file);
        }

        private void Save()
        {
            Directory.CreateDirectory(Path.GetDirectoryName(_file)!);
            File.WriteAllText(_file, _text.Text);
            using var key = Registry.CurrentUser.CreateSubKey(@"Software\Notes");
            key.SetValue("Width", Width);
            MessageBeep(0);
            MessageBox.Show("Saved!");
        }
    }
}
