# Blob Optimizers - read-only list of startup entries (registry Run keys, Startup folders, Store app startup tasks).
# Enabled/disabled state uses the same StartupApproved data as Task Manager.
$ErrorActionPreference = 'SilentlyContinue'

function Approved($hive, $sub, $name) {
    $v = (Get-ItemProperty -LiteralPath "$hive\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\$sub" -Name $name).$name
    if ($v -is [byte[]] -and $v.Length -gt 0) { return (($v[0] -band 1) -eq 0) }
    return $true
}

function Exe-Path([string]$cmd) {
    $cmd = [Environment]::ExpandEnvironmentVariables($cmd.Trim())
    if ($cmd.StartsWith('"')) { $e = $cmd.IndexOf('"', 1); if ($e -gt 1) { return $cmd.Substring(1, $e - 1) } }
    if ($cmd -match '^(?i)(.+?\.(exe|bat|cmd|com|lnk|vbs|ps1))(\s|$)') { return $Matches[1] }
    return ($cmd -split '\s+')[0]
}

function Publisher([string]$path) {
    if (-not $path -or -not (Test-Path -LiteralPath $path -PathType Leaf)) { return $null }
    $c = (Get-Item -LiteralPath $path).VersionInfo.CompanyName
    if ($c) { return $c.Trim() } else { return $null }
}

# Optional: per-app boot delays Windows records (readable only with administrator rights).
$impact = @{}
try {
    foreach ($e in Get-WinEvent -LogName 'Microsoft-Windows-Diagnostics-Performance/Operational' -FilterXPath '*[System[EventID=101]]' -MaxEvents 300 -ErrorAction Stop) {
        $x = [xml]$e.ToXml()
        $d = @{}; foreach ($n in $x.Event.EventData.Data) { $d[$n.Name] = $n.'#text' }
        $file = [string]$d['FileName']
        if ($file) {
            $ms = [int]$d['TotalTime']
            if (-not $impact.ContainsKey($file) -or $impact[$file] -lt $ms) { $impact[$file] = $ms }
        }
    }
    $impactAvailable = $true
} catch { $impactAvailable = $false }

$entries = New-Object System.Collections.ArrayList
function Add-Entry($source, $location, $name, $command, $enabled, $path, $scope) {
    $leaf = if ($path) { Split-Path -Leaf $path } else { $null }
    [void]$entries.Add([ordered]@{
        id        = "$source|$location|$name"
        source    = $source
        location  = $location
        name      = [string]$name
        command   = [string]$command
        path      = $path
        publisher = Publisher $path
        enabled   = [bool]$enabled
        scope     = $scope
        impactMs  = if ($leaf -and $impact.ContainsKey($leaf)) { $impact[$leaf] } else { $null }
    })
}

$runKeys = @(
    @{ hive = 'HKCU:'; path = 'Software\Microsoft\Windows\CurrentVersion\Run'; approved = 'Run'; scope = 'user'; loc = 'HKCU\Run' },
    @{ hive = 'HKLM:'; path = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Run'; approved = 'Run'; scope = 'machine'; loc = 'HKLM\Run' },
    @{ hive = 'HKLM:'; path = 'SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run'; approved = 'Run32'; scope = 'machine'; loc = 'HKLM\Run32' }
)
foreach ($k in $runKeys) {
    $props = Get-ItemProperty -LiteralPath "$($k.hive)\$($k.path)"
    if (-not $props) { continue }
    foreach ($p in $props.PSObject.Properties) {
        if ($p.Name -like 'PS*') { continue }
        Add-Entry 'registry' $k.loc $p.Name ([string]$p.Value) (Approved $k.hive $k.approved $p.Name) (Exe-Path ([string]$p.Value)) $k.scope
    }
}

$shell = New-Object -ComObject WScript.Shell
$folders = @(
    @{ path = [Environment]::GetFolderPath('Startup'); hive = 'HKCU:'; scope = 'user'; loc = 'UserStartup' },
    @{ path = [Environment]::GetFolderPath('CommonStartup'); hive = 'HKLM:'; scope = 'machine'; loc = 'CommonStartup' }
)
foreach ($f in $folders) {
    if (-not $f.path -or -not (Test-Path -LiteralPath $f.path)) { continue }
    foreach ($file in Get-ChildItem -LiteralPath $f.path -File -Force | Where-Object { $_.Name -ne 'desktop.ini' }) {
        $target = $file.FullName
        if ($file.Extension -eq '.lnk') { $t = $shell.CreateShortcut($file.FullName).TargetPath; if ($t) { $target = $t } }
        Add-Entry 'folder' $f.loc $file.Name $file.FullName (Approved $f.hive 'StartupFolder' $file.Name) $target $f.scope
    }
}

# Store app startup tasks.
$names = @{}
foreach ($s in Get-StartApps) { if ($s.AppID -match '^([^!]+)!') { $names[$Matches[1]] = $s.Name } }
$pubs = @{}
foreach ($a in Get-AppxPackage) { $pubs[$a.PackageFamilyName] = ([string]$a.Publisher -replace '^CN=("?)([^,"]+).*$', '$2') }
$root = 'HKCU:\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\SystemAppData'
foreach ($pkg in Get-ChildItem -LiteralPath $root) {
    foreach ($task in Get-ChildItem -LiteralPath $pkg.PSPath) {
        $state = (Get-ItemProperty -LiteralPath $task.PSPath).State
        if ($null -eq $state) { continue }
        $family = $pkg.PSChildName
        $display = if ($names[$family]) { $names[$family] } else { ($family -split '_')[0] }
        $pub = $pubs[$family]
        [void]$entries.Add([ordered]@{
            id = "store|$family|$($task.PSChildName)"; source = 'store'; location = $family; name = $display
            command = "Store app startup task ($($task.PSChildName))"; path = $null; publisher = $pub
            enabled = ($state -eq 2 -or $state -eq 4); scope = 'user'; impactMs = $null
            policy = ($state -eq 4 -or $state -eq 5)
        })
    }
}

[ordered]@{ impactAvailable = $impactAvailable; entries = @($entries) } | ConvertTo-Json -Depth 4 -Compress
