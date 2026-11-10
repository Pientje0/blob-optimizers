# Blob Optimizers - read-only detection of installed games from launcher data.
$ErrorActionPreference = 'SilentlyContinue'
$games = New-Object System.Collections.ArrayList
function Add-Game($source, $id, $name, $dir, $exe) {
    if (-not $dir -or -not (Test-Path -LiteralPath $dir -PathType Container)) { return }
    if ($exe -and -not (Test-Path -LiteralPath $exe -PathType Leaf)) { $exe = $null }
    [void]$games.Add([ordered]@{ id = "$source|$id"; source = $source; name = [string]$name; installDir = [string]$dir; exe = $exe })
}

# ---- Steam ----
$steam = (Get-ItemProperty 'HKCU:\Software\Valve\Steam').SteamPath
if ($steam) {
    $steam = $steam -replace '/', '\'
    $libs = @($steam)
    $vdf = Join-Path $steam 'steamapps\libraryfolders.vdf'
    if (Test-Path -LiteralPath $vdf) {
        foreach ($m in [regex]::Matches((Get-Content -LiteralPath $vdf -Raw), '"path"\s+"([^"]+)"')) { $libs += ($m.Groups[1].Value -replace '\\\\', '\') }
    }
    foreach ($lib in ($libs | Select-Object -Unique)) {
        foreach ($acf in Get-ChildItem -LiteralPath (Join-Path $lib 'steamapps') -Filter 'appmanifest_*.acf') {
            $t = Get-Content -LiteralPath $acf.FullName -Raw
            $appid = [regex]::Match($t, '"appid"\s+"(\d+)"').Groups[1].Value
            $name = [regex]::Match($t, '"name"\s+"([^"]+)"').Groups[1].Value
            $dir = [regex]::Match($t, '"installdir"\s+"([^"]+)"').Groups[1].Value
            if ($appid -eq '228980' -or $name -match '(?i)redistributable|proton|steam linux runtime|steamworks') { continue }
            Add-Game 'steam' $appid $name (Join-Path $lib "steamapps\common\$dir") $null
        }
    }
}

# ---- Epic Games ----
foreach ($item in Get-ChildItem -LiteralPath "$env:ProgramData\Epic\EpicGamesLauncher\Data\Manifests" -Filter '*.item') {
    $j = Get-Content -LiteralPath $item.FullName -Raw | ConvertFrom-Json
    if ($j.bIsIncompleteInstall -or ($j.AppCategories -and $j.AppCategories -notcontains 'games')) { continue }
    $exe = if ($j.LaunchExecutable) { Join-Path $j.InstallLocation $j.LaunchExecutable } else { $null }
    Add-Game 'epic' $j.AppName $j.DisplayName $j.InstallLocation $exe
}

# ---- GOG ----
foreach ($k in Get-ChildItem 'HKLM:\SOFTWARE\WOW6432Node\GOG.com\Games') {
    $p = Get-ItemProperty -LiteralPath $k.PSPath
    Add-Game 'gog' $k.PSChildName $p.gameName $p.path $p.exe
}

# ---- Riot Games ----
foreach ($drive in (Get-PSDrive -PSProvider FileSystem).Root) {
    $riot = Join-Path $drive 'Riot Games'
    if (-not (Test-Path -LiteralPath $riot)) { continue }
    Add-Game 'riot' 'valorant' 'VALORANT' (Join-Path $riot 'VALORANT\live') (Join-Path $riot 'VALORANT\live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe')
    Add-Game 'riot' 'lol' 'League of Legends' (Join-Path $riot 'League of Legends') (Join-Path $riot 'League of Legends\Game\League of Legends.exe')
}

# ---- Roblox (the newest installed version) ----
$roblox = @("$env:LOCALAPPDATA\Roblox\Versions", "${env:ProgramFiles(x86)}\Roblox\Versions") |
    ForEach-Object { Get-ChildItem -LiteralPath $_ -Filter 'RobloxPlayerBeta.exe' -Recurse -Depth 1 } |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($roblox) { Add-Game 'roblox' 'player' 'Roblox' $roblox.DirectoryName $roblox.FullName }

ConvertTo-Json -InputObject @($games) -Compress
