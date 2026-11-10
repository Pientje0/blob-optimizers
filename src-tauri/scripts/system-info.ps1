# Blob Optimizers - read-only power and virtual memory configuration.
$ErrorActionPreference = 'SilentlyContinue'

$plans = @(foreach ($line in (powercfg /list)) {
    if ($line -match '([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})\s+\((.+)\)\s*(\*)?\s*$') {
        [ordered]@{ guid = $Matches[1].ToLower(); name = $Matches[2]; active = [bool]$Matches[3] }
    }
})

# Current AC/DC values are the last two hex numbers in `powercfg /q` output (labels are localised).
function Get-Timeout($sub, $setting) {
    $vals = @((powercfg /q SCHEME_CURRENT $sub $setting) | Select-String -Pattern ':\s*0x([0-9a-fA-F]+)\s*$' | ForEach-Object { [Convert]::ToInt64($_.Matches[0].Groups[1].Value, 16) })
    if ($vals.Count -ge 2) { return @{ ac = [int]($vals[-2] / 60); dc = [int]($vals[-1] / 60) } }
    return @{ ac = $null; dc = $null }
}
$monitor = Get-Timeout 'SUB_VIDEO' 'VIDEOIDLE'
$sleep = Get-Timeout 'SUB_SLEEP' 'STANDBYIDLE'

$hib = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Power').HibernateEnabled
$hiberSize = $null
$hf = Get-Item -LiteralPath "$env:SystemDrive\hiberfil.sys" -Force
if ($hf) { $hiberSize = [uint64]$hf.Length }

$cs = Get-CimInstance Win32_ComputerSystem
$setting = @(Get-CimInstance Win32_PageFileSetting)
$usage = @(Get-CimInstance Win32_PageFileUsage)
$pf = [ordered]@{
    automatic   = [bool]$cs.AutomaticManagedPagefile
    files       = @(foreach ($u in $usage) {
        $s = $setting | Where-Object { $_.Name -eq $u.Name }
        [ordered]@{
            path          = [string]$u.Name
            allocatedMb   = [int]$u.AllocatedBaseSize
            currentUsageMb = [int]$u.CurrentUsage
            peakUsageMb   = [int]$u.PeakUsage
            initialMb     = if ($s) { [int]$s.InitialSize } else { $null }
            maximumMb     = if ($s) { [int]$s.MaximumSize } else { $null }
        }
    })
    ramMb       = [int]([uint64]$cs.TotalPhysicalMemory / 1MB)
}

[ordered]@{
    plans          = $plans
    monitorAc      = $monitor.ac
    monitorDc      = $monitor.dc
    sleepAc        = $sleep.ac
    sleepDc        = $sleep.dc
    hasBattery     = [bool](Get-CimInstance Win32_Battery)
    hibernate      = ($hib -eq 1)
    hiberfilBytes  = $hiberSize
    pagefile       = $pf
} | ConvertTo-Json -Depth 5 -Compress
