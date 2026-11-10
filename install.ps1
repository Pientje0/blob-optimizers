# Blob Optimizers launcher / installer.
#
#   irm https://raw.githubusercontent.com/Pientje09-abe/blob-optimizers/main/install.ps1 | iex
#
# Downloads the latest release's portable blob-optimizers.exe into
# %LOCALAPPDATA%\Programs\Blob Optimizers Portable (no admin needed), verifies its SHA-256
# against the checksum published with the release, and starts it. Run the same command again
# to update; if you already have the latest version it just starts it.

& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'Pientje09-abe/blob-optimizers'
    $dir = Join-Path $env:LOCALAPPDATA 'Programs\Blob Optimizers Portable'
    $exe = Join-Path $dir 'blob-optimizers.exe'
    $versionFile = Join-Path $dir 'version.txt'
    $headers = @{ 'User-Agent' = 'BlobOptimizers-Installer'; 'Accept' = 'application/vnd.github+json' }

    Write-Host ''
    Write-Host '  Blob Optimizers' -ForegroundColor Magenta
    Write-Host '  Windows PC & gaming optimization suite' -ForegroundColor DarkGray
    Write-Host ''

    if ([Environment]::OSVersion.Version.Major -lt 10) {
        Write-Host '  Blob Optimizers needs Windows 10 or 11.' -ForegroundColor Red
        return
    }

    try {
        $release = Invoke-RestMethod -UseBasicParsing -Headers $headers -Uri "https://api.github.com/repos/$repo/releases/latest"
    } catch {
        if (Test-Path $exe) {
            Write-Host '  Could not reach GitHub; starting the copy you already have.' -ForegroundColor Yellow
            Start-Process -FilePath $exe
            return
        }
        Write-Host "  Could not reach GitHub: $($_.Exception.Message)" -ForegroundColor Red
        return
    }

    $tag = [string]$release.tag_name
    $installed = if (Test-Path $versionFile) { (Get-Content $versionFile -Raw).Trim() } else { '' }

    if ((Test-Path $exe) -and $installed -eq $tag) {
        Write-Host "  Latest version ($tag) is already installed. Starting..." -ForegroundColor Green
        Start-Process -FilePath $exe
        return
    }

    $asset = $release.assets | Where-Object { $_.name -eq 'blob-optimizers.exe' } | Select-Object -First 1
    $sumAsset = $release.assets | Where-Object { $_.name -eq 'blob-optimizers.exe.sha256' } | Select-Object -First 1
    if (-not $asset -or -not $sumAsset) {
        Write-Host "  Release $tag has no portable download. Get the installer from https://github.com/$repo/releases" -ForegroundColor Red
        return
    }

    $running = Get-Process -Name 'blob-optimizers' -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
    if ($running) {
        Write-Host '  Blob Optimizers is running. Close it and run this command again to update.' -ForegroundColor Yellow
        return
    }

    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $tmp = Join-Path $dir 'blob-optimizers.exe.download'

    Write-Host "  Downloading $tag ($([math]::Round($asset.size / 1MB, 1)) MB)..."
    Invoke-WebRequest -UseBasicParsing -Headers @{ 'User-Agent' = 'BlobOptimizers-Installer' } -Uri $asset.browser_download_url -OutFile $tmp
    $sumContent = (Invoke-WebRequest -UseBasicParsing -Headers @{ 'User-Agent' = 'BlobOptimizers-Installer' } -Uri $sumAsset.browser_download_url).Content
    # Windows PowerShell 5.1 returns binary content types as bytes.
    if ($sumContent -is [byte[]]) { $sumContent = [Text.Encoding]::ASCII.GetString($sumContent) }
    $expected = ([string]$sumContent).Trim().Split(' ')[0].Trim().ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $tmp).Hash.ToLower()
    if ($expected -ne $actual) {
        Remove-Item $tmp -Force
        Write-Host '  Checksum mismatch: the download was not what the release published. Nothing was installed.' -ForegroundColor Red
        return
    }

    Move-Item -Force -Path $tmp -Destination $exe
    Set-Content -Path $versionFile -Value $tag -Encoding ASCII

    # Start menu shortcut so it can also be opened like any other app.
    try {
        $lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Blob Optimizers.lnk'
        $shell = New-Object -ComObject WScript.Shell
        $s = $shell.CreateShortcut($lnk)
        $s.TargetPath = $exe
        $s.WorkingDirectory = $dir
        $s.Description = 'Blob Optimizers'
        $s.Save()
    } catch { }

    $wv = 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    if (-not (Test-Path $wv) -and -not (Test-Path 'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}')) {
        Write-Host '  Note: Microsoft Edge WebView2 was not found. If the window stays blank, install it from https://go.microsoft.com/fwlink/p/?LinkId=2124703' -ForegroundColor Yellow
    }

    Write-Host "  Installed $tag to $dir" -ForegroundColor Green
    Write-Host '  Starting Blob Optimizers...'
    Start-Process -FilePath $exe
}
