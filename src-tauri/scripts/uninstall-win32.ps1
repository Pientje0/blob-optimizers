# Blob Optimizers - uninstall one desktop program using its own registered uninstaller.
# The uninstall command is read here from the program's registry entry; nothing is passed in from the UI
# except which entry to use.
$key = '__KEY__'
$name = '__NAME__'

$entry = Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
if (-not $entry) {
    "$name is no longer registered as installed. Nothing to do."
    '##PROGRESS 100 Already removed'
    exit 0
}

# Pick the least interactive official method.
$isMsi = $false
$guid = $null
if (([string]$entry.UninstallString + ' ' + [string]$entry.ModifyPath) -match '\{[0-9A-Fa-f-]{36}\}') { $guid = $Matches[0] }
if (($entry.WindowsInstaller -eq 1 -or [string]$entry.UninstallString -match '(?i)msiexec') -and $guid) {
    $isMsi = $true
    $file = "$env:SystemRoot\System32\msiexec.exe"
    $arguments = "/x $guid /passive /norestart"
} else {
    $cmdline = if ($entry.QuietUninstallString) { [string]$entry.QuietUninstallString } else { [string]$entry.UninstallString }
    $cmdline = $cmdline.Trim()
    if ($cmdline.StartsWith('"')) {
        $end = $cmdline.IndexOf('"', 1)
        $file = $cmdline.Substring(1, $end - 1)
        $arguments = $cmdline.Substring($end + 1).Trim()
    } elseif ($cmdline -match '^(?i)(.+?\.exe)(\s+.*)?$') {
        $file = $Matches[1]
        $arguments = ([string]$Matches[2]).Trim()
    } else {
        $parts = $cmdline.Split(' ', 2)
        $file = $parts[0]
        $arguments = if ($parts.Count -gt 1) { $parts[1] } else { '' }
    }
}

"Uninstalling $name"
"Running: $file $arguments"
'##PROGRESS 5 Starting uninstaller'
$started = Get-Date
try {
    if ($arguments) { $p = Start-Process -FilePath $file -ArgumentList $arguments -PassThru -ErrorAction Stop }
    else { $p = Start-Process -FilePath $file -PassThru -ErrorAction Stop }
} catch {
    if ($_.Exception.Message -match '(?i)cancel') { [Console]::Error.WriteLine('The administrator prompt was declined; nothing was removed.'); exit 1223 }
    [Console]::Error.WriteLine("Could not start the uninstaller: $($_.Exception.Message)")
    exit 1
}
$null = $p.Handle
'##PROGRESS 15 Waiting for the uninstaller (answer its window if it asks questions)'

# Many uninstallers hand off to a helper copy and exit immediately, so watch the registry entry and any
# uninstall helpers until the program is actually gone.
$helper = '(?i)unins|uninst|^au_$|^un_|^un[a-z]?$|setup|installer|update|msiexec'
$idle = 0
$reportedExit = $false
$deadline = (Get-Date).AddMinutes(15)
while ((Get-Date) -lt $deadline) {
    if (-not (Test-Path -LiteralPath $key)) {
        "$name was removed."
        '##PROGRESS 100 Removed'
        exit 0
    }
    if ($p.HasExited) {
        if (-not $reportedExit) {
            $reportedExit = $true
            $code = $p.ExitCode
            "Uninstaller exited with code $code"
            if ($isMsi) {
                switch ($code) {
                    0 { }
                    3010 { 'Windows needs a restart to finish removing this program.' }
                    1605 { 'Windows Installer reports the program is not installed.' }
                    1602 { [Console]::Error.WriteLine('The uninstall was cancelled.'); exit 1602 }
                    default { [Console]::Error.WriteLine("Windows Installer reported error $code."); exit $code }
                }
            }
        }
        $busy = @(Get-Process | Where-Object { $_.StartTime -ge $started.AddSeconds(-2) -and ($_.ProcessName -match $helper -or ($_.Path -and $_.Path -like "$env:TEMP*")) }).Count -gt 0
        if ($busy) { $idle = 0 } else { $idle++ }
        if ($idle -ge 20) { break }
    }
    Start-Sleep -Seconds 1
}
if (Test-Path -LiteralPath $key) {
    [Console]::Error.WriteLine("The uninstaller finished, but $name is still registered as installed. It may have been cancelled, or may need a restart to finish.")
    exit 2
}
"$name was removed."
exit 0
