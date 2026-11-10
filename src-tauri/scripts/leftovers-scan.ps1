# Blob Optimizers - read-only search for files and registry keys an uninstalled program left behind.
# Only exact name matches in well-known per-app locations are reported; nothing is deleted here.
$ErrorActionPreference = 'SilentlyContinue'
$appName = '__NAME__'
$publisher = '__PUBLISHER__'
$installLocation = '__LOCATION__'

function Clean-Name([string]$s) {
    $s = $s -replace '\s+for\s+\S+\s*$', ''                        # "Roblox Player for abesm"
    $s = $s -replace '\((?:x64|x86|64-bit|32-bit|User|Machine)\)', ''
    $s = $s -replace '\s+(?:version\s+)?v?\d+(?:[.\-]\d+)+.*$', ''    # trailing version numbers
    $s = $s -replace '\s+-\s+.*$', ''
    return $s.Trim()
}
function Clean-Publisher([string]$s) {
    $s = $s -replace ',?\s+(?:Inc\.?|LLC|Ltd\.?|Limited|GmbH|Corp\.?|Corporation|Co\.?,? Ltd\.?|S\.?A\.?|B\.?V\.?|AB|AG|Pte\.? Ltd\.?|Software)\s*$', ''
    return $s.Trim(' ', ',', '.')
}

$names = @($appName, (Clean-Name $appName)) | Where-Object { $_ -and $_.Length -ge 3 } | Select-Object -Unique
if ($installLocation) { $names += Split-Path -Leaf $installLocation.TrimEnd('\') }
$names = @($names | Where-Object { $_ -and $_.Length -ge 3 } | Select-Object -Unique)
$pubs = @($publisher, (Clean-Publisher $publisher)) | Where-Object { $_ -and $_.Length -ge 2 } | Select-Object -Unique

$found = New-Object System.Collections.ArrayList
function Add-Found($kind, $path, $reason) {
    if ($found | Where-Object { $_.path -eq $path }) { return }
    $size = 0; $items = 0
    if ($kind -eq 'folder') {
        $files = @(Get-ChildItem -LiteralPath $path -Recurse -Force -File)
        $items = $files.Count
        $size = ($files | Measure-Object Length -Sum).Sum
    }
    [void]$found.Add([ordered]@{ kind = $kind; path = $path; sizeBytes = [uint64]$size; items = [int]$items; reason = $reason })
}

if ($installLocation -and (Test-Path -LiteralPath $installLocation -PathType Container)) {
    Add-Found 'folder' (Resolve-Path -LiteralPath $installLocation).ProviderPath 'Install folder left behind'
}

$bases = @(
    @{ path = $env:ProgramFiles; label = 'Program Files' },
    @{ path = ${env:ProgramFiles(x86)}; label = 'Program Files (x86)' },
    @{ path = $env:ProgramData; label = 'ProgramData (shared app data)' },
    @{ path = $env:APPDATA; label = 'AppData\Roaming (your settings)' },
    @{ path = $env:LOCALAPPDATA; label = 'AppData\Local (your settings and cache)' },
    @{ path = (Join-Path $env:LOCALAPPDATA 'Programs'); label = 'AppData\Local\Programs' }
) | Where-Object { $_.path -and (Test-Path -LiteralPath $_.path) }

foreach ($b in $bases) {
    foreach ($n in $names) {
        $p = Join-Path $b.path $n
        if (Test-Path -LiteralPath $p -PathType Container) { Add-Found 'folder' $p "Folder named after the program in $($b.label)" }
        foreach ($pub in $pubs) {
            $p = Join-Path (Join-Path $b.path $pub) $n
            if (Test-Path -LiteralPath $p -PathType Container) { Add-Found 'folder' $p "Program folder under the publisher's folder in $($b.label)" }
        }
    }
}

$regBases = @('HKCU:\Software', 'HKLM:\SOFTWARE', 'HKLM:\SOFTWARE\WOW6432Node')
foreach ($rb in $regBases) {
    foreach ($n in $names) {
        $k = "$rb\$n"
        if (Test-Path -LiteralPath $k) { Add-Found 'registry' $k 'Settings key named after the program' }
        foreach ($pub in $pubs) {
            $k = "$rb\$pub\$n"
            if (Test-Path -LiteralPath $k) { Add-Found 'registry' $k "Settings key under the publisher's key" }
        }
    }
}

ConvertTo-Json -InputObject @($found) -Depth 3 -Compress
