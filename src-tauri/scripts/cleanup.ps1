# Blob Optimizers - storage cleanup. Mode 'scan' only measures; mode 'clean' deletes the selected categories.
# Only fixed cache/temp locations are touched, file by file (no wildcard deletes at a folder root).
$ErrorActionPreference = 'SilentlyContinue'
$mode = '__MODE__'
$selected = @(__SELECTED__)
$cutoff = (Get-Date).AddHours(-24)

$cats = @(
    @{ id = 'userTemp';   paths = @($env:TEMP); old = $true },
    @{ id = 'winTemp';    paths = @("$env:SystemRoot\Temp"); old = $true },
    @{ id = 'thumbnails'; paths = @("$env:LOCALAPPDATA\Microsoft\Windows\Explorer"); filter = 'thumbcache_*.db' },
    @{ id = 'shaders';    paths = @("$env:LOCALAPPDATA\D3DSCache", "$env:LOCALAPPDATA\AMD\DxCache", "$env:LOCALAPPDATA\AMD\DxcCache", "$env:LOCALAPPDATA\AMD\VkCache", "$env:LOCALAPPDATA\NVIDIA\DXCache", "$env:LOCALAPPDATA\NVIDIA\GLCache") },
    @{ id = 'errorReports'; paths = @("$env:ProgramData\Microsoft\Windows\WER\ReportArchive", "$env:ProgramData\Microsoft\Windows\WER\ReportQueue", "$env:LOCALAPPDATA\CrashDumps") },
    @{ id = 'updateLogs'; paths = @("$env:SystemRoot\Logs\CBS"); filter = '*.log'; old = $true }
)

function Get-Files($c) {
    foreach ($p in $c.paths) {
        if (-not $p -or -not (Test-Path -LiteralPath $p -PathType Container)) { continue }
        $items = if ($c.filter) { Get-ChildItem -LiteralPath $p -Filter $c.filter -File -Force } else { Get-ChildItem -LiteralPath $p -Recurse -File -Force }
        foreach ($f in $items) { if (-not $c.old -or $f.LastWriteTime -lt $cutoff) { $f } }
    }
}

function Get-RecycleBinSize {
    $shell = New-Object -ComObject Shell.Application
    $bin = $shell.Namespace(10)
    $size = 0; $count = 0
    foreach ($i in $bin.Items()) { $size += [uint64]$i.ExtendedProperty('Size'); $count++ }
    return @{ size = $size; count = $count }
}

if ($mode -eq 'scan') {
    $out = @(foreach ($c in $cats) {
        $files = @(Get-Files $c)
        [ordered]@{ id = $c.id; bytes = [uint64](($files | Measure-Object Length -Sum).Sum); files = $files.Count }
    })
    $rb = Get-RecycleBinSize
    $out += [ordered]@{ id = 'recycleBin'; bytes = [uint64]$rb.size; files = [int]$rb.count }
    ConvertTo-Json -InputObject $out -Compress
    exit 0
}

$freed = [uint64]0
$i = 0
foreach ($c in $cats) {
    if ($selected -notcontains $c.id) { continue }
    $i++
    "##PROGRESS $([int]($i / ($selected.Count + 1) * 100)) Cleaning $($c.id)"
    $done = 0; $skipped = 0; $bytes = [uint64]0
    foreach ($f in @(Get-Files $c)) {
        $len = $f.Length
        Remove-Item -LiteralPath $f.FullName -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $f.FullName) { $skipped++ } else { $done++; $bytes += $len }
    }
    # Remove folders left empty inside temp locations (deepest first), never the location itself.
    if (-not $c.filter) {
        foreach ($p in $c.paths) {
            if (-not (Test-Path -LiteralPath $p)) { continue }
            Get-ChildItem -LiteralPath $p -Recurse -Directory -Force | Sort-Object { $_.FullName.Length } -Descending | ForEach-Object {
                if (-not (Get-ChildItem -LiteralPath $_.FullName -Force | Select-Object -First 1)) { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue }
            }
        }
    }
    $freed += $bytes
    "$($c.id): removed $done files ($([math]::Round($bytes / 1MB, 1)) MB)$(if ($skipped) { ", skipped $skipped in use" })"
}
if ($selected -contains 'recycleBin') {
    $rb = Get-RecycleBinSize
    Clear-RecycleBin -Force -ErrorAction SilentlyContinue
    $freed += $rb.size
    "recycleBin: emptied $($rb.count) items ($([math]::Round($rb.size / 1MB, 1)) MB)"
}
"Freed about $([math]::Round($freed / 1MB, 1)) MB in total."
'##PROGRESS 100 Done'
