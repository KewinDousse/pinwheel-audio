$ErrorActionPreference = 'Continue'
$out = Join-Path $PWD 'probe-out'
New-Item -ItemType Directory -Force $out | Out-Null
Add-Type -AssemblyName System.Drawing, System.Windows.Forms, UIAutomationClient, UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class U {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static string Fg() {
    var h = GetForegroundWindow(); var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
    uint pid; GetWindowThreadProcessId(h, out pid);
    string name = "?"; try { name = System.Diagnostics.Process.GetProcessById((int)pid).ProcessName; } catch {}
    return "'" + sb + "' (" + name + ")";
  }
  public static void Click(int x, int y) { SetCursorPos(x, y); mouse_event(2,0,0,0,UIntPtr.Zero); mouse_event(4,0,0,0,UIntPtr.Zero); }
  public static void Key(byte vk, bool up) { keybd_event(vk, 0, up ? 2u : 0u, UIntPtr.Zero); }
  public static void Wheel(int delta) { mouse_event(0x800, 0, 0, delta, UIntPtr.Zero); }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public static void ClickWindow(string title) {
    var h = FindWindow(null, title); RECT r; GetWindowRect(h, out r);
    Click((r.L + r.R) / 2, r.B - 30);
  }
}
"@
$n = 0
function Shot($label) {
  $script:n++
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save((Join-Path $out ("{0:d2}-{1}.png" -f $script:n, $label)))
  $g.Dispose(); $bmp.Dispose()
  "[$label] foreground: $([U]::Fg())"
}
function FindByName($root, $name) {
  $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
  $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}
function ClickElement($el) {
  $r = $el.Current.BoundingRectangle
  [U]::Click([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2))
}

$dir = Join-Path $env:APPDATA 'Pinwheel'
New-Item -ItemType Directory -Force $dir | Out-Null
$settingsFile = Join-Path $dir 'settings.json'
'{"version":1,"language":"english","outputs":[{"id":"probe","name":"Probe output"}]}' | Set-Content $settingsFile
$exe = Resolve-Path 'target\release\pinwheel.exe'
"screen: $([System.Windows.Forms.Screen]::PrimaryScreen.Bounds)"

Start-Process $exe
Start-Sleep -Milliseconds 900
Shot 'started-notice'
Start-Sleep 2
"Start menu entry: $(Test-Path (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Pinwheel.lnk'))"
Start-Process $exe -ArgumentList '--settings'
Start-Sleep 8
Shot 'settings'

[U]::ClickWindow('Pinwheel settings')
Start-Sleep 1
"[before-uia] foreground: $([U]::Fg())"
[U]::Key(0x41, $false); [U]::Key(0x41, $true)
Start-Sleep 1
$root = [System.Windows.Automation.AutomationElement]::RootElement
$window = FindByName $root 'Pinwheel settings'
"window found: $($null -ne $window)"
[U]::Key(0x42, $false); [U]::Key(0x42, $true)
Start-Sleep 1
$set = FindByName $window 'Set…'
"Set button found: $($null -ne $set)"
if ($set) { ClickElement $set } else { [U]::Click(300, 250) }
Start-Sleep 1
Shot 'capture-started'
[U]::Key(0x11, $false); [U]::Key(0x12, $false); [U]::Key(0x31, $false)
Start-Sleep -Milliseconds 100
[U]::Key(0x31, $true); [U]::Key(0x12, $true); [U]::Key(0x11, $true)
Start-Sleep 2
Shot 'after-ctrl-alt-1'
"settings after capture: $(Get-Content $settingsFile -Raw)"

$change = FindByName $window 'Change…'
$again = if ($change) { $change } else { FindByName $window 'Set…' }
if ($again) { ClickElement $again }
Start-Sleep 1
[U]::Key(0x1B, $false); Start-Sleep -Milliseconds 50; [U]::Key(0x1B, $true)
Start-Sleep 1
Shot 'after-escape'
$again = FindByName $window 'Set…'
if ($again) { ClickElement $again }
Start-Sleep 17
Shot 'after-timeout'

# Win held long enough to repeat, as a physical key does, with wheel notches in the middle.
[U]::Click(900, 600)
Start-Sleep 1
[U]::Key(0x5B, $false)
for ($i = 0; $i -lt 10; $i++) { Start-Sleep -Milliseconds 33; [U]::Key(0x5B, $false) }
[U]::Wheel(120); Start-Sleep -Milliseconds 100; [U]::Wheel(120)
for ($i = 0; $i -lt 15; $i++) { Start-Sleep -Milliseconds 33; [U]::Key(0x5B, $false) }
[U]::Key(0x5B, $true)
Start-Sleep 2
Shot 'after-win-wheel-held'
[U]::Key(0x1B, $false); [U]::Key(0x1B, $true)
Start-Sleep 1

# Win tapped quickly around one notch.
[U]::Key(0x5B, $false); [U]::Wheel(-120); Start-Sleep -Milliseconds 50; [U]::Key(0x5B, $true)
Start-Sleep 2
Shot 'after-win-wheel-quick'
[U]::Key(0x1B, $false); [U]::Key(0x1B, $true)

# The settings window in the dark theme, scrolled to its end too.
$settingsProcess = Get-Process pinwheel | Where-Object { $_.MainWindowTitle -eq 'Pinwheel settings' }
$settingsProcess | Stop-Process
Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' AppsUseLightTheme 0
Start-Sleep 2
Start-Process $exe -ArgumentList '--settings'
Start-Sleep 8
Shot 'settings-dark'
[U]::SetCursorPos(400, 400) | Out-Null
for ($i = 0; $i -lt 10; $i++) { [U]::Wheel(-120); Start-Sleep -Milliseconds 50 }
Start-Sleep 1
Shot 'settings-dark-end'

$log = Join-Path $env:LOCALAPPDATA 'Pinwheel\logs\pinwheel.log'
if (Test-Path $log) { Copy-Item $log $out; "--- log"; Get-Content $log }
