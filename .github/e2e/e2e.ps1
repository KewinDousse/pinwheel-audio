# End-to-end checks on a Windows desktop: the real exe, real keys and real
# clicks. Runs on the CI's Windows runner, which has no audio device, so
# nothing here needs one. Screenshots and the log land in e2e-out/.
param([Parameter(Mandatory)][string]$Exe)

$ErrorActionPreference = 'Continue'
$out = Join-Path $PWD 'e2e-out'
New-Item -ItemType Directory -Force $out | Out-Null
Add-Type -AssemblyName System.Drawing, System.Windows.Forms, UIAutomationClient, UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class U {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public static string ForegroundProcess() {
    uint pid; GetWindowThreadProcessId(GetForegroundWindow(), out pid);
    try { return System.Diagnostics.Process.GetProcessById((int)pid).ProcessName; } catch { return "?"; }
  }
  // Paced like a hand on a mouse: egui hit-tests against the pass before,
  // so the pointer has to rest on the widget for a pass before pressing.
  public static void Click(int x, int y) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(150);
    mouse_event(2,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(60);
    mouse_event(4,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(60);
    mouse_event(1,1,0,0,UIntPtr.Zero);
  }
  public static void Key(byte vk, bool up) { keybd_event(vk, 0, up ? 2u : 0u, UIntPtr.Zero); }
  public static void Wheel(int delta) { mouse_event(0x800, 0, 0, delta, UIntPtr.Zero); }
}
"@

$failures = New-Object System.Collections.Generic.List[string]
function Check($name, $ok, $detail = '') {
  if ($ok) { "PASS  $name" } else { "FAIL  $name $detail"; $failures.Add($name) }
}

$title = 'Pinwheel settings'
$n = 0
function Shot($label) {
  $script:n++
  $h = [U]::FindWindow([NullString]::Value, $title)
  $r = New-Object U+RECT
  if ($h -eq [IntPtr]::Zero -or -not [U]::GetWindowRect($h, [ref]$r)) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $r.L = 0; $r.T = 0; $r.R = $b.Width; $r.B = $b.Height
  }
  $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $out ("{0:d2}-{1}.png" -f $script:n, $label)))
  $g.Dispose(); $bmp.Dispose()
}
function Press([byte[]]$keys) {
  foreach ($k in $keys) { [U]::Key($k, $false) }
  Start-Sleep -Milliseconds 80
  [array]::Reverse($keys)
  foreach ($k in $keys) { [U]::Key($k, $true) }
}
function Find($name) {
  $window = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
    [System.Windows.Automation.TreeScope]::Children,
    (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $title)))
  if (-not $window) { return $null }
  $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
    (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)))
}
function ClickOn($name) {
  $el = Find $name
  if (-not $el) { Write-Host "      (no element named '$name')"; return $false }
  $r = $el.Current.BoundingRectangle
  $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
  Write-Host "      click '$name' at $x,$y"
  [U]::Click($x, $y)
  Start-Sleep -Milliseconds 700
  $true
}
function Saved { Get-Content $settingsFile -Raw | ConvertFrom-Json }
$log = Join-Path $env:LOCALAPPDATA 'Pinwheel\logs\pinwheel.log'
function LogHas($text) { (Test-Path $log) -and (Select-String -Path $log -SimpleMatch $text -Quiet) }

$dir = Join-Path $env:APPDATA 'Pinwheel'
New-Item -ItemType Directory -Force $dir | Out-Null
$settingsFile = Join-Path $dir 'settings.json'
'{"version":1,"language":"english","outputs":[{"id":"e2e","name":"Test output"}]}' | Set-Content $settingsFile

# The background process.
Start-Process $Exe
Start-Sleep 3
Check 'background process runs' ([bool](Get-Process pinwheel -ErrorAction SilentlyContinue))
$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Pinwheel.lnk'
Check 'Start menu entry created' (Test-Path $shortcut)

# The settings window, each card open in turn.
Start-Process $Exe -ArgumentList '--settings'
Start-Sleep 8
Check 'settings window opens' ([U]::FindWindow([NullString]::Value, $title) -ne [IntPtr]::Zero)
Shot 'closed'
foreach ($card in '+  Add an output', '+ wheel changes the volume', 'General') {
  if (ClickOn $card) { Shot ("open-" + ($card -replace '[^a-z]', '')) }
}
ClickOn 'General' | Out-Null

# Shortcut capture: Ctrl+Alt+1 is saved.
ClickOn 'Set a key' | Out-Null
Check 'capture starts from the card' (LogHas 'shortcut capture started')
Shot 'capture'
Press 0x11, 0x12, 0x31
Start-Sleep 2
$hotkey = (Saved).outputs[0].hotkey
Check 'Ctrl+Alt+1 saved' ($hotkey -and $hotkey.ctrl -and $hotkey.alt -and $hotkey.key -eq 0x31) ($hotkey | ConvertTo-Json -Compress)
Shot 'captured'

# Escape cancels a new capture and keeps the shortcut.
ClickOn 'Change' | Out-Null
Press 0x1B
Start-Sleep 1
Check 'Escape cancels the capture' (LogHas 'shortcut capture ended: Cancelled')
Check 'shortcut kept after Escape' (((Saved).outputs[0].hotkey.key) -eq 0x31)

# The saved shortcut works from another window.
[U]::Click(10, 10)
Start-Sleep 1
Press 0x11, 0x12, 0x31
Start-Sleep 1
Check 'saved shortcut fires' (LogHas 'shortcut pressed for "Test output"')

# Win held long enough to repeat, as a physical key does, with wheel notches.
[U]::Click(10, 10)
Start-Sleep 1
[U]::Key(0x5B, $false)
for ($i = 0; $i -lt 10; $i++) { Start-Sleep -Milliseconds 33; [U]::Key(0x5B, $false) }
[U]::Wheel(120); Start-Sleep -Milliseconds 100; [U]::Wheel(120)
for ($i = 0; $i -lt 15; $i++) { Start-Sleep -Milliseconds 33; [U]::Key(0x5B, $false) }
[U]::Key(0x5B, $true)
Start-Sleep 2
$foreground = [U]::ForegroundProcess()
Check 'Start stays closed after Win + wheel' ($foreground -notmatch 'StartMenuExperienceHost|SearchHost|SearchApp') $foreground
if ($foreground -match 'StartMenuExperienceHost|SearchHost|SearchApp') { Press 0x1B }

# The same window in the dark theme.
Get-Process pinwheel | Where-Object { $_.MainWindowTitle -eq $title } | Stop-Process
Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' AppsUseLightTheme 0
Start-Sleep 2
Start-Process $Exe -ArgumentList '--settings'
Start-Sleep 8
if (ClickOn 'Test output') { Shot 'dark-output-open' }
if (ClickOn '+ wheel changes the volume') { Shot 'dark-wheel-open' }

if (Test-Path $log) { Copy-Item $log $out }
Get-Process pinwheel -ErrorAction SilentlyContinue | Stop-Process
if ($failures.Count) { "$($failures.Count) check(s) failed"; exit 1 }
"all checks passed"
