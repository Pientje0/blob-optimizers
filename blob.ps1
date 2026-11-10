<#
    Blob Optimizers  -  Windows PC & gaming optimization script
    https://github.com/Pientje09-abe/blob-optimizers

    Run it (installs nothing, no compiled .exe):

        irm https://raw.githubusercontent.com/Pientje09-abe/blob-optimizers/main/blob.ps1 | iex

    Everything this script does is in plain sight below. Each tweak is a short list of
    registry values or service changes, and every change can be reversed from the same menu.
    It never touches Microsoft Defender, the firewall or Windows Update services.
#>

[CmdletBinding()]
param(
    # Testing / automation helpers (end users don't need these):
    [switch]$List,                 # Print the catalogue and exit (no admin, no changes)
    [switch]$NonInteractive,       # Load functions/data only; don't show UI or self-elevate
    [switch]$Console,              # Force the text menu instead of the window
    [switch]$SelfTest,             # Build the window without showing it, report, and exit
    [string]$BlobUrl = 'https://raw.githubusercontent.com/Pientje09-abe/blob-optimizers/main/blob.ps1'
)

$ErrorActionPreference = 'Stop'
$script:Version = '0.4.0'

# ------------------------------------------------------------------ helpers ---

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
        [Security.Principal.WindowsBuiltinRole]::Administrator)
}

function Write-Head($text) {
    Write-Host ''
    Write-Host "  $text" -ForegroundColor Magenta
    Write-Host ('  ' + ('-' * $text.Length)) -ForegroundColor DarkGray
}

$script:HiveMap = @{ HKCU = 'HKCU:'; HKLM = 'HKLM:' }

function Get-RegValue($hive, $path, $name) {
    $full = Join-Path $script:HiveMap[$hive] $path
    try { (Get-ItemProperty -Path $full -Name $name -ErrorAction Stop).$name } catch { $null }
}

function Set-RegValue($hive, $path, $name, $type, $value) {
    $full = Join-Path $script:HiveMap[$hive] $path
    if (-not (Test-Path $full)) { New-Item -Path $full -Force | Out-Null }
    New-ItemProperty -Path $full -Name $name -PropertyType $type -Value $value -Force | Out-Null
}

function Remove-RegValue($hive, $path, $name) {
    $full = Join-Path $script:HiveMap[$hive] $path
    Remove-ItemProperty -Path $full -Name $name -ErrorAction SilentlyContinue
}

# Apply / test / describe a single operation (reg value, reg-value removal, service start type).
function Invoke-Op($op) {
    switch ($op.K) {
        'reg'    { Set-RegValue $op.Hive $op.Path $op.Name $op.Type $op.Value }
        'regdel' { Remove-RegValue $op.Hive $op.Path $op.Name }
        'svc'    {
            Set-Service -Name $op.Name -StartupType $op.Start -ErrorAction Stop
            if ($op.Start -eq 'Disabled') { Stop-Service -Name $op.Name -Force -ErrorAction SilentlyContinue }
        }
    }
}

function Test-Op($op) {
    switch ($op.K) {
        'reg'    { "$(Get-RegValue $op.Hive $op.Path $op.Name)" -eq "$($op.Value)" }
        'regdel' { $null -eq (Get-RegValue $op.Hive $op.Path $op.Name) }
        'svc'    {
            $s = Get-Service -Name $op.Name -ErrorAction SilentlyContinue
            $s -and $s.StartType -eq $op.Start
        }
        default  { $false }
    }
}

function Show-Op($op) {
    switch ($op.K) {
        'reg'    { "$($op.Hive)\$($op.Path) > $($op.Name) = $($op.Value)" }
        'regdel' { "$($op.Hive)\$($op.Path) > $($op.Name) removed" }
        'svc'    { "Service $($op.Name) > $($op.Start)" }
    }
}

# A tweak is "applied" when all of its On operations currently match.
function Test-Applied($t) { -not ($t.On | Where-Object { -not (Test-Op $_) }) }

function Set-Tweak($t, [switch]$Revert) {
    $ops = if ($Revert) { $t.Off } else { $t.On }
    foreach ($op in $ops) { Invoke-Op $op }
}

# --------------------------------------------------------------- catalogue ---
# Each tweak: Id, Name, Cat(egory), Rec(ommended), Desc, Caveat, On[], Off[].
# On  = operations that apply the tweak.   Off = operations that restore the Windows default.

function Reg($hive, $path, $name, $type, $value) { @{ K = 'reg'; Hive = $hive; Path = $path; Name = $name; Type = $type; Value = $value } }
function RegDel($hive, $path, $name)             { @{ K = 'regdel'; Hive = $hive; Path = $path; Name = $name } }
function Svc($name, $start)                      { @{ K = 'svc'; Name = $name; Start = $start } }

$ADV   = 'Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced'
$CDM   = 'Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager'
$MMSP  = 'SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile'
$GAMES = "$MMSP\Tasks\Games"
$GCS   = 'System\GameConfigStore'
$PERS  = 'Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'

$script:Tweaks = @(
    # ---- Performance ----
    [pscustomobject]@{ Id='startup_delay'; Cat='Performance'; Rec=$true; Name='Remove the startup app delay'
        Desc='Launch startup apps immediately instead of ~10s after sign-in.'; Caveat=''
        On =@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize' 'StartupDelayInMSec' 'DWord' 0 )
        Off=@( RegDel 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize' 'StartupDelayInMSec' ) }
    [pscustomobject]@{ Id='menu_delay'; Cat='Performance'; Rec=$true; Name='Open menus faster'
        Desc='Shorten the submenu hover delay from 400ms to 100ms.'; Caveat=''
        On =@( Reg 'HKCU' 'Control Panel\Desktop' 'MenuShowDelay' 'String' '100' )
        Off=@( Reg 'HKCU' 'Control Panel\Desktop' 'MenuShowDelay' 'String' '400' ) }
    [pscustomobject]@{ Id='taskbar_anim'; Cat='Performance'; Rec=$false; Name='Turn off taskbar animations'
        Desc='Taskbar buttons appear without sliding animations.'; Caveat=''
        On =@( Reg 'HKCU' $ADV 'TaskbarAnimations' 'DWord' 0 )
        Off=@( Reg 'HKCU' $ADV 'TaskbarAnimations' 'DWord' 1 ) }
    [pscustomobject]@{ Id='transparency'; Cat='Performance'; Rec=$false; Name='Turn off transparency effects'
        Desc='Solid colours instead of blur in Start, taskbar and windows.'; Caveat=''
        On =@( Reg 'HKCU' $PERS 'EnableTransparency' 'DWord' 0 )
        Off=@( Reg 'HKCU' $PERS 'EnableTransparency' 'DWord' 1 ) }
    [pscustomobject]@{ Id='background_apps'; Cat='Performance'; Rec=$false; Name='Stop Store apps running in the background'
        Desc='Store apps no longer run background tasks when unused.'; Caveat='Mail, messaging and alarm apps may stop updating until opened.'
        On =@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications' 'GlobalUserDisabled' 'DWord' 1 )
        Off=@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications' 'GlobalUserDisabled' 'DWord' 0 ) }
    [pscustomobject]@{ Id='power_throttling'; Cat='Performance'; Rec=$false; Name='Turn off power throttling'
        Desc='Background apps get full CPU speed instead of being slowed to save power.'; Caveat='Reduces battery life on laptops.'
        On =@( Reg 'HKLM' 'SYSTEM\CurrentControlSet\Control\Power\PowerThrottling' 'PowerThrottlingOff' 'DWord' 1 )
        Off=@( RegDel 'HKLM' 'SYSTEM\CurrentControlSet\Control\Power\PowerThrottling' 'PowerThrottlingOff' ) }
    [pscustomobject]@{ Id='sysmain'; Cat='Performance'; Rec=$false; Name='Turn off SysMain (Superfetch)'
        Desc='Stops SysMain preloading apps into spare memory.'; Caveat='Usually best left on; apps can open slower without it.'
        On =@( Svc 'SysMain' 'Disabled' )
        Off=@( Svc 'SysMain' 'Automatic' ) }

    # ---- Latency ----
    [pscustomobject]@{ Id='mouse_accel'; Cat='Latency'; Rec=$true; Name='Turn off mouse acceleration'
        Desc="Disable 'Enhance pointer precision' so the cursor moves the same for the same hand movement."; Caveat='The desktop pointer may feel slower; adjust DPI to taste.'
        On =@( Reg 'HKCU' 'Control Panel\Mouse' 'MouseSpeed' 'String' '0'; Reg 'HKCU' 'Control Panel\Mouse' 'MouseThreshold1' 'String' '0'; Reg 'HKCU' 'Control Panel\Mouse' 'MouseThreshold2' 'String' '0' )
        Off=@( Reg 'HKCU' 'Control Panel\Mouse' 'MouseSpeed' 'String' '1'; Reg 'HKCU' 'Control Panel\Mouse' 'MouseThreshold1' 'String' '6'; Reg 'HKCU' 'Control Panel\Mouse' 'MouseThreshold2' 'String' '10' ) }
    [pscustomobject]@{ Id='priority_sep'; Cat='Latency'; Rec=$false; Name='Favour the foreground app'
        Desc='Short, variable CPU time slices with maximum boost for the active window (Win32PrioritySeparation 0x26).'; Caveat='Background tasks get less CPU while a game is focused.'
        On =@( Reg 'HKLM' 'SYSTEM\CurrentControlSet\Control\PriorityControl' 'Win32PrioritySeparation' 'DWord' 0x26 )
        Off=@( Reg 'HKLM' 'SYSTEM\CurrentControlSet\Control\PriorityControl' 'Win32PrioritySeparation' 'DWord' 2 ) }
    [pscustomobject]@{ Id='system_resp'; Cat='Latency'; Rec=$false; Name='Reserve less CPU for background tasks'
        Desc='Lowers the multimedia scheduler reserve from 20% to 10% while games/media run.'; Caveat='Background tasks get less CPU while you play.'
        On =@( Reg 'HKLM' $MMSP 'SystemResponsiveness' 'DWord' 10 )
        Off=@( Reg 'HKLM' $MMSP 'SystemResponsiveness' 'DWord' 20 ) }
    [pscustomobject]@{ Id='net_throttle'; Cat='Latency'; Rec=$false; Name='Turn off network throttling'
        Desc='Removes the limit Windows puts on network traffic while audio/video plays.'; Caveat='Mainly helps downloads during playback; rarely changes in-game ping.'
        On =@( Reg 'HKLM' $MMSP 'NetworkThrottlingIndex' 'DWord' 0xFFFFFFFF )
        Off=@( Reg 'HKLM' $MMSP 'NetworkThrottlingIndex' 'DWord' 10 ) }
    [pscustomobject]@{ Id='games_priority'; Cat='Latency'; Rec=$false; Name='Prioritise games in the multimedia scheduler'
        Desc='Raises CPU/GPU/IO priority MMCSS gives to games that register with it.'; Caveat='Only affects games that use MMCSS; effect is usually small.'
        On =@( Reg 'HKLM' $GAMES 'GPU Priority' 'DWord' 8; Reg 'HKLM' $GAMES 'Priority' 'DWord' 6; Reg 'HKLM' $GAMES 'Scheduling Category' 'String' 'High'; Reg 'HKLM' $GAMES 'SFIO Priority' 'String' 'High' )
        Off=@( Reg 'HKLM' $GAMES 'Priority' 'DWord' 2; Reg 'HKLM' $GAMES 'Scheduling Category' 'String' 'Medium'; Reg 'HKLM' $GAMES 'SFIO Priority' 'String' 'Normal' ) }

    # ---- Gaming ----
    [pscustomobject]@{ Id='game_mode'; Cat='Gaming'; Rec=$true; Name='Turn on Game Mode'
        Desc='Windows prioritises the running game and pauses driver installs/restarts while it runs.'; Caveat=''
        On =@( Reg 'HKCU' 'Software\Microsoft\GameBar' 'AutoGameModeEnabled' 'DWord' 1 )
        Off=@( Reg 'HKCU' 'Software\Microsoft\GameBar' 'AutoGameModeEnabled' 'DWord' 0 ) }
    [pscustomobject]@{ Id='no_dvr'; Cat='Gaming'; Rec=$true; Name='Turn off background recording (Game DVR)'
        Desc='Stops Windows recording gameplay in the background (saves GPU encoder time and disk writes).'; Caveat='Win+Alt+R stops working. OBS/Medal/vendor tools are unaffected.'
        On =@( Reg 'HKCU' $GCS 'GameDVR_Enabled' 'DWord' 0; Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\GameDVR' 'AppCaptureEnabled' 'DWord' 0 )
        Off=@( RegDel 'HKCU' $GCS 'GameDVR_Enabled'; RegDel 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\GameDVR' 'AppCaptureEnabled' ) }
    [pscustomobject]@{ Id='hags'; Cat='Gaming'; Rec=$false; Name='Hardware-accelerated GPU scheduling'
        Desc='Lets the GPU manage its own memory scheduling (needed by some features like DLSS Frame Generation).'; Caveat='Needs a supported GPU/driver. Turn back off if games stutter or crash. Restart required.'
        On =@( Reg 'HKLM' 'SYSTEM\CurrentControlSet\Control\GraphicsDrivers' 'HwSchMode' 'DWord' 2 )
        Off=@( RegDel 'HKLM' 'SYSTEM\CurrentControlSet\Control\GraphicsDrivers' 'HwSchMode' ) }
    [pscustomobject]@{ Id='gamebar_tips'; Cat='Gaming'; Rec=$true; Name='Hide Game Bar startup tips'
        Desc="Stops the 'Press Win+G' popup when a game launches."; Caveat=''
        On =@( Reg 'HKCU' 'Software\Microsoft\GameBar' 'ShowStartupPanel' 'DWord' 0 )
        Off=@( Reg 'HKCU' 'Software\Microsoft\GameBar' 'ShowStartupPanel' 'DWord' 1 ) }

    # ---- Privacy ----
    [pscustomobject]@{ Id='ad_id'; Cat='Privacy'; Rec=$true; Name='Turn off advertising ID'
        Desc='Apps can no longer use your advertising ID for personalised ads.'; Caveat=''
        On =@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' 'Enabled' 'DWord' 0 )
        Off=@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo' 'Enabled' 'DWord' 1 ) }
    [pscustomobject]@{ Id='tailored'; Cat='Privacy'; Rec=$true; Name='Turn off tailored experiences'
        Desc='Stops Microsoft using diagnostic data to personalise tips and ads.'; Caveat=''
        On =@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\Privacy' 'TailoredExperiencesWithDiagnosticDataEnabled' 'DWord' 0 )
        Off=@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\Privacy' 'TailoredExperiencesWithDiagnosticDataEnabled' 'DWord' 1 ) }
    [pscustomobject]@{ Id='activity'; Cat='Privacy'; Rec=$true; Name='Turn off activity history'
        Desc='Stops Windows recording and uploading which apps and files you use.'; Caveat=''
        On =@( Reg 'HKLM' 'SOFTWARE\Policies\Microsoft\Windows\System' 'PublishUserActivities' 'DWord' 0 )
        Off=@( RegDel 'HKLM' 'SOFTWARE\Policies\Microsoft\Windows\System' 'PublishUserActivities' ) }
    [pscustomobject]@{ Id='start_suggestions'; Cat='Privacy'; Rec=$true; Name='Hide suggestions in Start'
        Desc='Removes promoted app suggestions from the Start menu.'; Caveat=''
        On =@( Reg 'HKCU' $CDM 'SystemPaneSuggestionsEnabled' 'DWord' 0 )
        Off=@( Reg 'HKCU' $CDM 'SystemPaneSuggestionsEnabled' 'DWord' 1 ) }
    [pscustomobject]@{ Id='web_search'; Cat='Privacy'; Rec=$true; Name='Turn off web results in Start search'
        Desc='Start search shows only local apps, files and settings (no Bing).'; Caveat=''
        On =@( Reg 'HKCU' 'Software\Policies\Microsoft\Windows\Explorer' 'DisableSearchBoxSuggestions' 'DWord' 1 )
        Off=@( RegDel 'HKCU' 'Software\Policies\Microsoft\Windows\Explorer' 'DisableSearchBoxSuggestions' ) }

    # ---- Interface ----
    [pscustomobject]@{ Id='file_ext'; Cat='Interface'; Rec=$true; Name='Show file extensions'
        Desc='Show endings such as .exe or .pdf so you can tell file types apart.'; Caveat=''
        On =@( Reg 'HKCU' $ADV 'HideFileExt' 'DWord' 0 )
        Off=@( Reg 'HKCU' $ADV 'HideFileExt' 'DWord' 1 ) }
    [pscustomobject]@{ Id='hidden_files'; Cat='Interface'; Rec=$false; Name='Show hidden files and folders'
        Desc='Show files and folders marked as hidden, such as AppData.'; Caveat=''
        On =@( Reg 'HKCU' $ADV 'Hidden' 'DWord' 1 )
        Off=@( Reg 'HKCU' $ADV 'Hidden' 'DWord' 2 ) }
    [pscustomobject]@{ Id='taskbar_left'; Cat='Interface'; Rec=$false; Name='Align taskbar icons to the left'
        Desc='Move Start and app icons to the left edge, like Windows 10.'; Caveat=''
        On =@( Reg 'HKCU' $ADV 'TaskbarAl' 'DWord' 0 )
        Off=@( Reg 'HKCU' $ADV 'TaskbarAl' 'DWord' 1 ) }
    [pscustomobject]@{ Id='dark_mode'; Cat='Interface'; Rec=$false; Name='Use dark mode'
        Desc='Switch Windows and apps that follow the system theme to dark mode.'; Caveat=''
        On =@( Reg 'HKCU' $PERS 'AppsUseLightTheme' 'DWord' 0; Reg 'HKCU' $PERS 'SystemUsesLightTheme' 'DWord' 0 )
        Off=@( Reg 'HKCU' $PERS 'AppsUseLightTheme' 'DWord' 1; Reg 'HKCU' $PERS 'SystemUsesLightTheme' 'DWord' 1 ) }
    [pscustomobject]@{ Id='end_task'; Cat='Interface'; Rec=$true; Name="Add 'End task' to the taskbar right-click menu"
        Desc='Force-close a frozen app by right-clicking its taskbar button.'; Caveat=''
        On =@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\TaskbarDeveloperSettings' 'TaskbarEndTask' 'DWord' 1 )
        Off=@( Reg 'HKCU' 'Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\TaskbarDeveloperSettings' 'TaskbarEndTask' 'DWord' 0 ) }
)

# Store apps offered for removal (safe, user-installed extras). Core/system apps are never listed.
$script:DebloatList = @(
    'Microsoft.BingNews', 'Microsoft.BingWeather', 'Microsoft.GamingApp', 'Microsoft.GetHelp',
    'Microsoft.Getstarted', 'Microsoft.MicrosoftOfficeHub', 'Microsoft.MicrosoftSolitaireCollection',
    'Microsoft.People', 'Microsoft.PowerAutomateDesktop', 'Microsoft.Todos', 'Microsoft.WindowsFeedbackHub',
    'Microsoft.WindowsMaps', 'Microsoft.ZuneMusic', 'Microsoft.ZuneVideo', 'Microsoft.XboxApp',
    'Microsoft.MixedReality.Portal', 'Microsoft.SkypeApp', 'Clipchamp.Clipchamp', 'MicrosoftTeams',
    'Microsoft.549981C3F5F10'   # Cortana
)

# ------------------------------------------------------------------ actions ---

function New-RestorePoint {
    Write-Head 'Create a restore point'
    try {
        Enable-ComputerRestore -Drive "$env:SystemDrive\" -ErrorAction SilentlyContinue
        Checkpoint-Computer -Description 'Before Blob Optimizers' -RestorePointType 'MODIFY_SETTINGS'
        Write-Host '  Restore point created.' -ForegroundColor Green
    } catch {
        Write-Host "  Could not create a restore point: $($_.Exception.Message)" -ForegroundColor Yellow
        Write-Host '  (System Protection may be off. You can turn it on in System > About > System protection.)' -ForegroundColor DarkGray
    }
}

function Show-Category($cat) {
    while ($true) {
        $items = @($script:Tweaks | Where-Object Cat -eq $cat)
        Write-Head "$cat tweaks"
        for ($i = 0; $i -lt $items.Count; $i++) {
            $t = $items[$i]
            $on = Test-Applied $t
            $box = if ($on) { '[x]' } else { '[ ]' }
            $col = if ($on) { 'Green' } else { 'Gray' }
            $rec = if ($t.Rec) { ' *' } else { '' }
            Write-Host ("  {0,2}. {1} {2}{3}" -f ($i + 1), $box, $t.Name, $rec) -ForegroundColor $col
        }
        Write-Host ''
        Write-Host '  Enter a number to toggle, d <n> for details, a = apply recommended, b = back' -ForegroundColor DarkGray
        $in = (Read-Host '  >').Trim()
        if ($in -eq 'b' -or $in -eq '') { return }
        if ($in -eq 'a') {
            foreach ($t in $items | Where-Object { $_.Rec -and -not (Test-Applied $_) }) {
                Set-Tweak $t; Write-Host "  Applied: $($t.Name)" -ForegroundColor Green
            }
            continue
        }
        if ($in -match '^d\s*(\d+)$') {
            $t = $items[[int]$Matches[1] - 1]
            if ($t) {
                Write-Host ''
                Write-Host "  $($t.Name)" -ForegroundColor White
                Write-Host "  $($t.Desc)" -ForegroundColor Gray
                if ($t.Caveat) { Write-Host "  Note: $($t.Caveat)" -ForegroundColor Yellow }
                Write-Host '  Applies:' -ForegroundColor DarkGray
                $t.On | ForEach-Object { Write-Host "    $(Show-Op $_)" -ForegroundColor DarkGray }
            }
            continue
        }
        if ($in -match '^\d+$') {
            $t = $items[[int]$in - 1]
            if (-not $t) { continue }
            if (Test-Applied $t) {
                Set-Tweak $t -Revert; Write-Host "  Reverted to Windows default: $($t.Name)" -ForegroundColor Yellow
            } else {
                if ($t.Caveat) { Write-Host "  Note: $($t.Caveat)" -ForegroundColor Yellow }
                Set-Tweak $t; Write-Host "  Applied: $($t.Name)" -ForegroundColor Green
            }
        }
    }
}

function Show-Debloat {
    Write-Head 'Debloat apps'
    $present = @()
    foreach ($id in $script:DebloatList) {
        $pkg = Get-AppxPackage -Name $id -ErrorAction SilentlyContinue
        if ($pkg) { $present += [pscustomobject]@{ Id = $id; Name = ($pkg | Select-Object -First 1).Name } }
    }
    if (-not $present.Count) { Write-Host '  None of the removable apps are installed.' -ForegroundColor Green; return }
    for ($i = 0; $i -lt $present.Count; $i++) {
        Write-Host ("  {0,2}. {1}" -f ($i + 1), ($present[$i].Id -replace '^Microsoft\.', '')) -ForegroundColor Gray
    }
    Write-Host ''
    Write-Host '  Enter numbers to remove (e.g. 1,3,5), all, or b = back' -ForegroundColor DarkGray
    $in = (Read-Host '  >').Trim()
    if ($in -eq 'b' -or $in -eq '') { return }
    $chosen = if ($in -eq 'all') { $present } else {
        $in -split '[,\s]+' | Where-Object { $_ -match '^\d+$' } | ForEach-Object { $present[[int]$_ - 1] } | Where-Object { $_ }
    }
    foreach ($app in $chosen) {
        try {
            Get-AppxPackage -Name $app.Id -ErrorAction SilentlyContinue | Remove-AppxPackage -ErrorAction Stop
            Write-Host "  Removed: $($app.Id)" -ForegroundColor Green
        } catch {
            Write-Host "  Could not remove $($app.Id): $($_.Exception.Message)" -ForegroundColor Yellow
        }
    }
}

function Set-PowerPlan {
    Write-Head 'Power plan'
    Write-Host '  1. Balanced (recommended for most PCs)'
    Write-Host '  2. High performance'
    Write-Host '  3. Ultimate Performance (desktops; may not exist on all PCs)'
    Write-Host '  b. Back'
    switch ((Read-Host '  >').Trim()) {
        '1' { powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e; Write-Host '  Balanced active.' -ForegroundColor Green }
        '2' { powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c; Write-Host '  High performance active.' -ForegroundColor Green }
        '3' {
            powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61 2>$null | Out-Null
            powercfg /setactive e9a42b02-d5df-448d-aa00-03f14749eb61
            Write-Host '  Ultimate Performance active (if supported).' -ForegroundColor Green
        }
    }
}

# --------------------------------------------------------------------- menu ---

function Show-Menu {
    while ($true) {
        Write-Host ''
        Write-Host '  BLOB OPTIMIZERS' -ForegroundColor Magenta
        Write-Host "  Windows optimization script  v$script:Version" -ForegroundColor DarkGray
        Write-Host ''
        Write-Host '   1. Performance tweaks'
        Write-Host '   2. Latency tweaks'
        Write-Host '   3. Gaming tweaks'
        Write-Host '   4. Privacy tweaks'
        Write-Host '   5. Interface tweaks'
        Write-Host '   6. Debloat apps'
        Write-Host '   7. Power plan'
        Write-Host '   8. Create a restore point'
        Write-Host '   9. Apply all recommended (*) tweaks'
        Write-Host '   0. Exit'
        Write-Host ''
        Write-Host '  * = recommended. Applied tweaks show [x]; pick one again to revert it.' -ForegroundColor DarkGray
        switch ((Read-Host '  >').Trim()) {
            '1' { Show-Category 'Performance' }
            '2' { Show-Category 'Latency' }
            '3' { Show-Category 'Gaming' }
            '4' { Show-Category 'Privacy' }
            '5' { Show-Category 'Interface' }
            '6' { Show-Debloat }
            '7' { Set-PowerPlan }
            '8' { New-RestorePoint }
            '9' {
                foreach ($t in $script:Tweaks | Where-Object { $_.Rec -and -not (Test-Applied $_) }) {
                    Set-Tweak $t; Write-Host "  Applied: $($t.Name)" -ForegroundColor Green
                }
            }
            '0' { return }
            'exit' { return }
        }
    }
}

# --------------------------------------------------------------------- main ---

# ----------------------------------------------------------------- window ---
# A graphical window (WPF): category sidebar on the left, one check-box per tweak on the right.
# A ticked box = the tweak is applied right now. Tick/untick then "Apply changes" to make it so.

$script:Categories = @('Performance', 'Latency', 'Gaming', 'Privacy', 'Interface', 'Debloat')

function New-BlobWindow {
    Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

    $xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="Blob Optimizers" Height="720" Width="1040" WindowStartupLocation="CenterScreen"
        Background="#0b0b10" FontFamily="Segoe UI" TextOptions.TextFormattingMode="Display">
  <Window.Resources>
    <Style TargetType="Button">
      <Setter Property="Background" Value="#1a1a24"/>
      <Setter Property="Foreground" Value="#e8e8ef"/>
      <Setter Property="BorderBrush" Value="#2a2a38"/>
      <Setter Property="Padding" Value="14,7"/>
      <Setter Property="Margin" Value="6,0,0,0"/>
      <Setter Property="Cursor" Value="Hand"/>
      <Setter Property="Template">
        <Setter.Value>
          <ControlTemplate TargetType="Button">
            <Border Background="{TemplateBinding Background}" CornerRadius="8" Padding="{TemplateBinding Padding}">
              <ContentPresenter HorizontalAlignment="Center" VerticalAlignment="Center"/>
            </Border>
          </ControlTemplate>
        </Setter.Value>
      </Setter>
    </Style>
    <Style x:Key="Accent" TargetType="Button" BasedOn="{StaticResource {x:Type Button}}">
      <Setter Property="Background" Value="#7c5cff"/>
      <Setter Property="Foreground" Value="White"/>
    </Style>
    <Style x:Key="Nav" TargetType="Button" BasedOn="{StaticResource {x:Type Button}}">
      <Setter Property="Background" Value="Transparent"/>
      <Setter Property="Foreground" Value="#9a9aab"/>
      <Setter Property="Margin" Value="8,2"/>
      <Setter Property="Padding" Value="12,9"/>
      <Setter Property="HorizontalContentAlignment" Value="Left"/>
    </Style>
    <Style TargetType="CheckBox">
      <Setter Property="Foreground" Value="#e8e8ef"/>
      <Setter Property="Margin" Value="0,7"/>
      <Setter Property="FontSize" Value="13"/>
    </Style>
  </Window.Resources>
  <Grid>
    <Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="*"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>
    <Border Grid.Row="0" Background="#111119" Padding="18,14">
      <StackPanel Orientation="Horizontal">
        <TextBlock Text="BLOB OPTIMIZERS" Foreground="#b48cff" FontSize="17" FontWeight="Bold"/>
        <TextBlock x:Name="Sub" Foreground="#6b6b7b" FontSize="12" VerticalAlignment="Bottom" Margin="10,0,0,2"/>
      </StackPanel>
    </Border>
    <Grid Grid.Row="1">
      <Grid.ColumnDefinitions><ColumnDefinition Width="196"/><ColumnDefinition Width="*"/></Grid.ColumnDefinitions>
      <Border Grid.Column="0" Background="#0e0e16"><StackPanel x:Name="Nav" Margin="0,10"/></Border>
      <ScrollViewer Grid.Column="1" VerticalScrollBarVisibility="Auto" Padding="22,16">
        <StackPanel x:Name="Body"/>
      </ScrollViewer>
    </Grid>
    <Border Grid.Row="2" Background="#111119" Padding="14">
      <Grid>
        <TextBlock x:Name="Status" Foreground="#6b6b7b" FontSize="12" VerticalAlignment="Center"/>
        <StackPanel Orientation="Horizontal" HorizontalAlignment="Right">
          <Button x:Name="BtnRecommended" Content="Select recommended"/>
          <Button x:Name="BtnRestore" Content="Create restore point"/>
          <Button x:Name="BtnApply" Content="Apply changes" Style="{StaticResource Accent}"/>
        </StackPanel>
      </Grid>
    </Border>
  </Grid>
</Window>
"@

    $win = [Windows.Markup.XamlReader]::Load((New-Object System.Xml.XmlNodeReader ([xml]$xaml)))
    $find = { param($n) $win.FindName($n) }
    $nav = & $find 'Nav'; $body = & $find 'Body'
    (& $find 'Sub').Text = "Windows optimization  .  v$script:Version"

    # Per-category content panels (built once, shown one at a time) and their check boxes.
    $panels = @{}
    $script:GuiBoxes = @{}   # tweak Id  -> CheckBox
    $script:GuiDebloat = @{} # appx Id   -> CheckBox

    foreach ($cat in $script:Categories) {
        $panel = New-Object Windows.Controls.StackPanel
        $panel.Visibility = 'Collapsed'
        $intro = New-Object Windows.Controls.TextBlock
        $intro.Foreground = '#8a8a9a'; $intro.FontSize = 12; $intro.Margin = '0,0,0,10'; $intro.TextWrapping = 'Wrap'
        $intro.Text = if ($cat -eq 'Debloat') {
            'Tick the apps you want to remove, then Apply changes. Only safe, user-facing Store apps are listed.'
        } else {
            'A ticked box means the tweak is applied now. Tick or untick, then Apply changes. Everything is reversible.'
        }
        [void]$panel.Children.Add($intro)

        if ($cat -eq 'Debloat') {
            foreach ($id in ($script:DebloatList | Sort-Object)) {
                if (-not (Get-AppxPackage -Name $id -ErrorAction SilentlyContinue)) { continue }
                $cb = New-Object Windows.Controls.CheckBox
                $cb.Content = ($id -replace '^Microsoft\.', ''); $cb.Tag = $id
                $script:GuiDebloat[$id] = $cb
                [void]$panel.Children.Add($cb)
            }
            if ($panel.Children.Count -eq 1) {
                $none = New-Object Windows.Controls.TextBlock
                $none.Text = 'None of the removable apps are installed.'; $none.Foreground = '#5bd38a'
                [void]$panel.Children.Add($none)
            }
        } else {
            foreach ($t in $script:Tweaks | Where-Object Cat -eq $cat) {
                $cb = New-Object Windows.Controls.CheckBox
                $cb.Tag = $t
                $cb.IsChecked = [bool](Test-Applied $t)
                $head = if ($t.Rec) { "$($t.Name)   (recommended)" } else { $t.Name }
                $sp = New-Object Windows.Controls.StackPanel
                $tb = New-Object Windows.Controls.TextBlock; $tb.Text = $head; $tb.Foreground = '#e8e8ef'
                $db = New-Object Windows.Controls.TextBlock
                $db.Text = $t.Desc; $db.Foreground = '#7a7a8a'; $db.FontSize = 11.5; $db.TextWrapping = 'Wrap'
                [void]$sp.Children.Add($tb); [void]$sp.Children.Add($db)
                if ($t.Caveat) {
                    $cv = New-Object Windows.Controls.TextBlock
                    $cv.Text = "Note: $($t.Caveat)"; $cv.Foreground = '#d9a441'; $cv.FontSize = 11; $cv.TextWrapping = 'Wrap'
                    [void]$sp.Children.Add($cv)
                }
                $cb.Content = $sp
                $script:GuiBoxes[$t.Id] = $cb
                [void]$panel.Children.Add($cb)
            }
        }
        $panels[$cat] = $panel
        [void]$body.Children.Add($panel)
    }

    # Sidebar navigation: show one category panel at a time.
    $script:GuiCurrent = $script:Categories[0]
    $show = {
        param($cat)
        $script:GuiCurrent = $cat
        foreach ($k in $panels.Keys) { $panels[$k].Visibility = if ($k -eq $cat) { 'Visible' } else { 'Collapsed' } }
    }
    foreach ($cat in $script:Categories) {
        $b = New-Object Windows.Controls.Button
        $b.Content = $cat; $b.Style = $win.FindResource('Nav'); $b.Tag = $cat
        $b.Add_Click({ & $show $this.Tag }.GetNewClosure())
        [void]$nav.Children.Add($b)
    }
    & $show $script:GuiCurrent

    [pscustomobject]@{
        Window      = $win
        Status      = (& $find 'Status')
        BtnApply    = (& $find 'BtnApply')
        BtnRec      = (& $find 'BtnRecommended')
        BtnRestore  = (& $find 'BtnRestore')
    }
}

function Show-Gui {
    $ui = New-BlobWindow

    $ui.BtnRec.Add_Click({
        if ($script:GuiCurrent -eq 'Debloat') { return }
        foreach ($t in $script:Tweaks | Where-Object Cat -eq $script:GuiCurrent) {
            if ($t.Rec) { $script:GuiBoxes[$t.Id].IsChecked = $true }
        }
    })

    $ui.BtnRestore.Add_Click({
        $ui.Status.Text = 'Creating restore point...'
        try {
            Enable-ComputerRestore -Drive "$env:SystemDrive\" -ErrorAction SilentlyContinue
            Checkpoint-Computer -Description 'Before Blob Optimizers' -RestorePointType 'MODIFY_SETTINGS'
            $ui.Status.Text = 'Restore point created.'
        } catch {
            $ui.Status.Text = 'Could not create a restore point (System Protection may be off).'
        }
    })

    $ui.BtnApply.Add_Click({
        $cat = $script:GuiCurrent
        if ($cat -eq 'Debloat') {
            $remove = $script:GuiDebloat.Values | Where-Object { $_.IsChecked }
            if (-not $remove) { $ui.Status.Text = 'Nothing ticked to remove.'; return }
            if ([Windows.MessageBox]::Show("Remove $($remove.Count) app(s)? This can't be undone from here (reinstall from the Store).",
                    'Confirm', 'YesNo', 'Warning') -ne 'Yes') { return }
            $done = 0
            foreach ($cb in $remove) {
                try { Get-AppxPackage -Name $cb.Tag -EA SilentlyContinue | Remove-AppxPackage -EA Stop; $cb.IsEnabled = $false; $cb.IsChecked = $false; $done++ } catch {}
            }
            $ui.Status.Text = "Removed $done app(s)."
            return
        }
        $toApply = @(); $toRevert = @()
        foreach ($t in $script:Tweaks | Where-Object Cat -eq $cat) {
            $want = [bool]$script:GuiBoxes[$t.Id].IsChecked
            $now = [bool](Test-Applied $t)
            if ($want -and -not $now) { $toApply += $t } elseif (-not $want -and $now) { $toRevert += $t }
        }
        if (-not $toApply -and -not $toRevert) { $ui.Status.Text = 'No changes in this section.'; return }
        if ([Windows.MessageBox]::Show("Apply $($toApply.Count) and revert $($toRevert.Count) tweak(s) in $cat?",
                'Confirm', 'YesNo', 'Question') -ne 'Yes') { return }
        foreach ($t in $toApply) { try { Set-Tweak $t } catch {} }
        foreach ($t in $toRevert) { try { Set-Tweak $t -Revert } catch {} }
        foreach ($t in $script:Tweaks | Where-Object Cat -eq $cat) { $script:GuiBoxes[$t.Id].IsChecked = [bool](Test-Applied $t) }
        $ui.Status.Text = "Applied $($toApply.Count), reverted $($toRevert.Count). Some changes need a sign-out or restart."
    })

    if ($SelfTest) {
        Write-Host "GUI built: $($script:GuiBoxes.Count) tweak boxes, $($script:GuiDebloat.Count) debloat boxes." -ForegroundColor Green
        $ui.Window.Close()
        return
    }
    [void]$ui.Window.ShowDialog()
}

# --------------------------------------------------------------------- main ---

if ($List) {
    $script:Tweaks | Sort-Object Cat | Format-Table @{ n = 'Category'; e = 'Cat' }, @{ n = 'Recommended'; e = { if ($_.Rec) { 'yes' } else { '' } } }, Name -AutoSize
    Write-Host "Total: $($script:Tweaks.Count) tweaks, $($script:DebloatList.Count) debloat targets." -ForegroundColor Cyan
    return
}

if ($NonInteractive) { return }

# Elevate first (unless we're only building the window for a self-test).
if (-not $SelfTest -and -not (Test-Admin)) {
    Write-Host 'Blob Optimizers needs administrator rights. Approve the Windows prompt...' -ForegroundColor Yellow
    try {
        Start-Process powershell.exe -Verb RunAs -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', "irm $BlobUrl | iex")
    } catch {
        Write-Host 'Elevation was cancelled. Re-run and approve the prompt to make changes.' -ForegroundColor Red
    }
    return
}

# Prefer the window; fall back to the text menu if WPF can't load or -Console was passed.
if ($Console) {
    Show-Menu
} else {
    try {
        Show-Gui
    } catch {
        Write-Host "Could not open the window ($($_.Exception.Message)). Falling back to the text menu." -ForegroundColor Yellow
        Show-Menu
    }
}
if (-not $SelfTest) {
    Write-Host ''
    Write-Host '  Done. Some changes take effect after you sign out or restart.' -ForegroundColor Cyan
}
