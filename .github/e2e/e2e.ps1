# End-to-end checks on a Windows desktop: the real exe, real keys and real
# clicks. Runs on the CI's Windows runner, which has no audio device, so
# nothing here needs one. Screenshots and the log land in e2e-out/.
param([Parameter(Mandatory)][string]$Exe)

# It overwrites the Pinwheel settings and switches Windows to the dark theme.
if (-not $env:CI) { throw 'e2e.ps1 changes the settings of this session: run it on a CI runner only.' }

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
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
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
  public static void MiddleClick() {
    mouse_event(0x20,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(60);
    mouse_event(0x40,0,0,0,UIntPtr.Zero);
  }
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
function WindowRect {
  $r = New-Object U+RECT
  [U]::GetWindowRect([U]::FindWindow([NullString]::Value, $title), [ref]$r) | Out-Null
  $r
}
function ClickOn($name) {
  $el = Find $name
  if (-not $el) { Write-Host "      (no element named '$name')"; return $false }
  # Past the window's edge, it is scrolled to with the wheel, as by hand.
  # 40 px leave out the title bar.
  for ($i = 0; $i -lt 15; $i++) {
    $w = WindowRect
    $b = $el.Current.BoundingRectangle
    if ($b.Bottom -gt $w.B) { $notch = -120 } elseif ($b.Top -lt $w.T + 40) { $notch = 120 } else { break }
    [U]::SetCursorPos([int](($w.L + $w.R) / 2), [int](($w.T + $w.B) / 2)) | Out-Null
    [U]::Wheel($notch)
    Start-Sleep -Milliseconds 300
    $el = Find $name
  }
  $r = $el.Current.BoundingRectangle
  $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
  Write-Host "      click '$name' at $x,$y"
  [U]::Click($x, $y)
  Start-Sleep -Milliseconds 700
  $true
}
function ClickAway($when) {
  [U]::Click(10, 10)
  Start-Sleep 1
  Check "another window has the focus $when" ([U]::ForegroundProcess() -ne 'pinwheel')
}
function MoveSettings($x, $y, $width = 0, $height = 0) {
  # SWP_NOZORDER, plus SWP_NOSIZE without a size.
  $flags = if ($width) { 4 } else { 5 }
  [U]::SetWindowPos([U]::FindWindow([NullString]::Value, $title), [IntPtr]::Zero, $x, $y, $width, $height, $flags) | Out-Null
  Start-Sleep 1
}
# Windows cascades new windows, sometimes behind the taskbar, where a click
# opens Search. Clear of the taskbar, and of (10, 10) where ClickAway clicks.
function OpenSettings([switch]$WherePlaced) {
  Start-Process $Exe -ArgumentList '--settings'
  Start-Sleep 8
  if (-not $WherePlaced) { MoveSettings 200 0 }
}
function Saved { Get-Content $settingsFile -Raw | ConvertFrom-Json }
$log = Join-Path $env:LOCALAPPDATA 'Pinwheel\logs\pinwheel.log'
function LogHas($text) { (Test-Path $log) -and (Select-String -Path $log -SimpleMatch $text -Quiet) }

$dir = Join-Path $env:APPDATA 'Pinwheel'
New-Item -ItemType Directory -Force $dir | Out-Null
$settingsFile = Join-Path $dir 'settings.json'
# Enough outputs that the About card, open, goes past the window's bottom.
'{"version":1,"language":"english","outputs":[{"id":"e2e","name":"Test output"},{"id":"e2e-2","name":"Speakers"},{"id":"e2e-3","name":"Headset"},{"id":"e2e-4","name":"Monitor"}]}' | Set-Content $settingsFile

# The background process.
Start-Process $Exe
Start-Sleep 3
Check 'background process runs' ([bool](Get-Process pinwheel -ErrorAction SilentlyContinue))
$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Pinwheel.lnk'
Check 'Start menu entry created' (Test-Path $shortcut)

# The settings window, each card open in turn.
OpenSettings
Check 'settings window opens' ([U]::FindWindow([NullString]::Value, $title) -ne [IntPtr]::Zero)
Shot 'closed'
# What each card shows only when open. The runner has no audio device.
$cards = [ordered]@{
  '+  Add an output'            = 'No output connected.'
  'Next output'                 = 'Switches to the next connected output of the list.'
  '+ scroll changes the volume' = 'Win + middle click: mute'
  'On-screen display'           = 'Hide over full-screen apps and games'
  'General'                     = 'Open folder'
  'About Pinwheel'              = 'Report a problem'
}
$previous = $null
foreach ($card in $cards.Keys) {
  if (ClickOn $card) { Shot ("open-" + ($card -replace '[^a-z]', '')) }
  Check "'$card' opens" ([bool](Find $cards[$card]))
  if ($previous) { Check "'$card' closes the card open before" (-not (Find $previous)) }
  $previous = $cards[$card]
}
# The last card, open, goes past the window's bottom with four outputs.
$row = Find 'Report a problem'
$bottom = if ($row) { (WindowRect).B - $row.Current.BoundingRectangle.Bottom } else { -1 }
Check 'the open card scrolls into view' ($bottom -gt 0) "last row bottom $bottom px from the window's"
ClickOn 'About Pinwheel' | Out-Null

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
ClickAway 'before the shortcut'
Press 0x11, 0x12, 0x31
Start-Sleep 1
Check 'saved shortcut fires' (LogHas 'shortcut pressed for "Test output"')

# The "next output" shortcut, Ctrl+Alt+2, set from its card.
ClickOn 'Next output' | Out-Null
ClickOn 'Set' | Out-Null
Press 0x11, 0x12, 0x32
Start-Sleep 2
$hotkey = (Saved).next_output_hotkey
Check 'next output Ctrl+Alt+2 saved' ($hotkey -and $hotkey.ctrl -and $hotkey.alt -and $hotkey.key -eq 0x32) ($hotkey | ConvertTo-Json -Compress)
Shot 'next-output'
ClickOn 'Next output' | Out-Null
ClickAway 'before the next output shortcut'
Press 0x11, 0x12, 0x32
Start-Sleep 1
Check 'next output shortcut fires' (LogHas 'next output shortcut pressed')

# Win alone opens Start, so that Start staying closed below means something.
$startMenu = 'StartMenuExperienceHost|SearchHost|SearchApp'
ClickAway 'before Win alone'
Press 0x5B
Start-Sleep 2
$foreground = [U]::ForegroundProcess()
Check 'Win alone opens Start' ($foreground -match $startMenu) $foreground
Press 0x1B
Start-Sleep 1

# Each volume change or mute the hook passes on fails on the runner, which
# has no audio device: the failures in the log show that it took the event.
function AudioCalls { if (Test-Path $log) { @(Select-String -Path $log -SimpleMatch 'audio call failed').Count } else { 0 } }

# Win held long enough to repeat, as a physical key does, around `$use`.
# Windows does not open Start when Shift is held with Win, so Start is
# only checked without it.
function WithWinHeld($name, [scriptblock]$use, [switch]$Shift, [switch]$LeftToOthers) {
  ClickAway "before $name"
  $calls = AudioCalls
  if ($Shift) { [U]::Key(0x10, $false) }
  [U]::Key(0x5B, $false)
  for ($i = 0; $i -lt 10; $i++) { Start-Sleep -Milliseconds 33; [U]::Key(0x5B, $false) }
  & $use
  for ($i = 0; $i -lt 15; $i++) { Start-Sleep -Milliseconds 33; [U]::Key(0x5B, $false) }
  [U]::Key(0x5B, $true)
  if ($Shift) { [U]::Key(0x10, $true) }
  Start-Sleep 2
  if ($LeftToOthers) {
    Check "$name left to other apps" ((AudioCalls) -eq $calls)
  } else {
    Check "$name reaches Pinwheel" ((AudioCalls) -gt $calls)
  }
  if ($Shift) { return }
  $foreground = [U]::ForegroundProcess()
  Check "Start stays closed after $name" ($foreground -notmatch $startMenu) $foreground
  if ($foreground -match $startMenu) { Press 0x1B }
}
WithWinHeld 'Win + wheel' { [U]::Wheel(120); Start-Sleep -Milliseconds 100; [U]::Wheel(120) }
WithWinHeld 'Win + Shift + wheel' { [U]::Wheel(120) } -Shift
WithWinHeld 'Win + middle click' { [U]::MiddleClick() }

# Win + Shift + scroll turned off: Shift has a path of its own, which
# follows its switch. The settings are written as the settings window
# would, then the background process told to reload them (WM_APP + 2).
function SetFineStep($on) {
  $s = Saved
  $s | Add-Member -NotePropertyName fine_step -NotePropertyValue $on -Force
  $s | ConvertTo-Json -Depth 10 | Set-Content $settingsFile
  [U]::PostMessage([U]::FindWindow('Pinwheel.Main', [NullString]::Value), 0x8002, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
  Start-Sleep 2
}
SetFineStep $false
WithWinHeld 'Win + Shift + wheel, turned off,' { [U]::Wheel(120) } -Shift -LeftToOthers
SetFineStep $true

# Closed while minimized, as from the taskbar, the window keeps its normal
# place for the next opening, here in the dark theme.
$placeFile = Join-Path $env:LOCALAPPDATA 'Pinwheel\window.txt'
$moving = Get-Date
MoveSettings 260 20 720 660
$before = WindowRect
Start-Sleep 2
Check 'window place saved after a move' ((Test-Path $placeFile) -and (Get-Item $placeFile).LastWriteTime -ge $moving)
$settingsProcess = Get-Process pinwheel | Where-Object { $_.MainWindowTitle -eq $title }
# SW_MINIMIZE
[U]::ShowWindow([U]::FindWindow([NullString]::Value, $title), 6) | Out-Null
Start-Sleep 2
$settingsProcess | ForEach-Object { $_.CloseMainWindow() | Out-Null }
$settingsProcess | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
Check 'settings window closes' ([U]::FindWindow([NullString]::Value, $title) -eq [IntPtr]::Zero)
Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' AppsUseLightTheme 0
OpenSettings -WherePlaced
$after = WindowRect
$moved = "from $($before.L),$($before.T) $($before.R - $before.L)x$($before.B - $before.T) to $($after.L),$($after.T) $($after.R - $after.L)x$($after.B - $after.T)"
Write-Host "      window $moved"
$same = [Math]::Abs($after.L - $before.L) -le 2 -and [Math]::Abs($after.T - $before.T) -le 2 -and
  [Math]::Abs(($after.R - $after.L) - ($before.R - $before.L)) -le 2 -and [Math]::Abs(($after.B - $after.T) - ($before.B - $before.T)) -le 2
Check 'the window opens where it was closed' $same $moved
if (ClickOn 'Test output') { Shot 'dark-output-open' }
if (ClickOn '+ scroll changes the volume') { Shot 'dark-wheel-open' }
if (ClickOn 'On-screen display') { Shot 'dark-display-open' }

Check 'opening time logged' (LogHas 'settings window shown')
if (Test-Path $log) {
  Select-String -Path $log -SimpleMatch 'settings window shown' | ForEach-Object { Write-Host "      $($_.Line)" }
  Copy-Item $log $out
}
Get-Process pinwheel -ErrorAction SilentlyContinue | Stop-Process
if ($failures.Count) { "$($failures.Count) check(s) failed"; exit 1 }
"all checks passed"
