# Blob Optimizers - read-only hardware & Windows configuration detection.
# Emits one compact JSON document on stdout. Nothing in this script modifies the system.
$ErrorActionPreference = 'SilentlyContinue'

function Bytes-ToUInt64($v) {
    if ($null -eq $v) { return $null }
    if ($v -is [byte[]]) { if ($v.Length -ge 8) { return [BitConverter]::ToUInt64($v, 0) } else { return $null } }
    return [uint64]$v
}

# ---- Windows ----
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$osCim = Get-CimInstance Win32_OperatingSystem
$build = [int]$cv.CurrentBuildNumber
$productName = [string]$cv.ProductName
if ($build -ge 22000) { $productName = $productName -replace 'Windows 10', 'Windows 11' }
$windows = [ordered]@{
    productName    = $productName
    displayVersion = [string]$cv.DisplayVersion
    build          = "$($cv.CurrentBuildNumber).$($cv.UBR)"
    edition        = [string]$cv.EditionID
    architecture   = [string]$osCim.OSArchitecture
    installDate    = if ($osCim.InstallDate) { $osCim.InstallDate.ToString('yyyy-MM-dd') } else { $null }
    computerName   = $env:COMPUTERNAME
    lastBoot       = if ($osCim.LastBootUpTime) { $osCim.LastBootUpTime.ToString('s') } else { $null }
}

# ---- CPU ----
$c = Get-CimInstance Win32_Processor | Select-Object -First 1
$cpu = [ordered]@{
    name          = ([string]$c.Name).Trim()
    manufacturer  = [string]$c.Manufacturer
    cores         = [int]$c.NumberOfCores
    threads       = [int]$c.NumberOfLogicalProcessors
    maxClockMhz   = [int]$c.MaxClockSpeed
    l2CacheKb     = [int]$c.L2CacheSize
    l3CacheKb     = [int]$c.L3CacheSize
    socket        = [string]$c.SocketDesignation
    virtualization = [bool]$c.VirtualizationFirmwareEnabled
}

# ---- GPU ----
# Win32_VideoController.AdapterRAM is a 32-bit value capped at 4 GB, so read the driver's real VRAM size from the registry.
$vram = @{}
Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}' | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath
    if ($p.DriverDesc) {
        $size = Bytes-ToUInt64 $p.'HardwareInformation.qwMemorySize'
        if (-not $size) { $size = Bytes-ToUInt64 $p.'HardwareInformation.MemorySize' }
        if ($size) { $vram[[string]$p.DriverDesc] = $size }
    }
}
$gpus = @(foreach ($g in Get-CimInstance Win32_VideoController) {
    $name = [string]$g.Name
    $mem = $vram[$name]
    if (-not $mem -and $g.AdapterRAM) { $mem = [uint64][uint32]$g.AdapterRAM }
    [ordered]@{
        name          = $name
        manufacturer  = [string]$g.AdapterCompatibility
        vramBytes     = $mem
        driverVersion = [string]$g.DriverVersion
        driverDate    = if ($g.DriverDate) { $g.DriverDate.ToString('yyyy-MM-dd') } else { $null }
        resolution    = if ($g.CurrentHorizontalResolution) { "$($g.CurrentHorizontalResolution) x $($g.CurrentVerticalResolution)" } else { $null }
        refreshRate   = if ($g.CurrentRefreshRate) { [int]$g.CurrentRefreshRate } else { $null }
        status        = [string]$g.Status
    }
})

# ---- Memory ----
$modules = @(foreach ($m in Get-CimInstance Win32_PhysicalMemory) {
    [ordered]@{
        capacityBytes = [uint64]$m.Capacity
        speedMhz      = [int]$m.Speed
        configuredMhz = [int]$m.ConfiguredClockSpeed
        manufacturer  = ([string]$m.Manufacturer).Trim()
        partNumber    = ([string]$m.PartNumber).Trim()
        slot          = [string]$m.DeviceLocator
    }
})
$memory = [ordered]@{
    totalBytes = [uint64]$osCim.TotalVisibleMemorySize * 1024
    modules    = $modules
}

# ---- Storage ----
$volumesByDisk = @{}
$allVolumes = @{}
foreach ($v in Get-Volume) { if ($v.DriveLetter) { $allVolumes[[string]$v.DriveLetter] = $v } }
foreach ($part in Get-Partition | Where-Object { $_.DriveLetter }) {
    $vol = $allVolumes[[string]$part.DriveLetter]
    if (-not $vol) { continue }
    $key = [string]$part.DiskNumber
    if (-not $volumesByDisk.ContainsKey($key)) { $volumesByDisk[$key] = New-Object System.Collections.ArrayList }
    [void]$volumesByDisk[$key].Add([ordered]@{
        letter     = "$($part.DriveLetter):"
        label      = [string]$vol.FileSystemLabel
        fileSystem = [string]$vol.FileSystem
        sizeBytes  = [uint64]$vol.Size
        freeBytes  = [uint64]$vol.SizeRemaining
    })
}
$disks = @(foreach ($d in Get-PhysicalDisk | Sort-Object DeviceId) {
    $media = [string]$d.MediaType
    if ($media -eq 'Unspecified' -and [string]$d.BusType -eq 'NVMe') { $media = 'SSD' }
    $rel = $d | Get-StorageReliabilityCounter
    [ordered]@{
        number      = [int]$d.DeviceId
        model       = ([string]$d.FriendlyName).Trim()
        mediaType   = $media
        busType     = [string]$d.BusType
        sizeBytes   = [uint64]$d.Size
        health      = [string]$d.HealthStatus
        temperature = if ($rel -and $rel.Temperature) { [int]$rel.Temperature } else { $null }
        wearPercent = if ($rel -and $null -ne $rel.Wear) { [int]$rel.Wear } else { $null }
        volumes     = @($volumesByDisk[[string]$d.DeviceId])
    }
})

# ---- Motherboard / BIOS ----
$bb = Get-CimInstance Win32_BaseBoard
$bios = Get-CimInstance Win32_BIOS
$cs = Get-CimInstance Win32_ComputerSystem
$fw = $null
try { $fw = if (Confirm-SecureBootUEFI) { 'UEFI (Secure Boot on)' } else { 'UEFI (Secure Boot off)' } } catch { $fw = $null }
if (-not $fw) { $fw = if ($env:firmware_type) { $env:firmware_type } else { $null } }
$board = [ordered]@{
    manufacturer   = [string]$bb.Manufacturer
    product        = [string]$bb.Product
    systemModel    = "$($cs.Manufacturer) $($cs.Model)".Trim()
    biosVendor     = [string]$bios.Manufacturer
    biosVersion    = [string]$bios.SMBIOSBIOSVersion
    biosDate       = if ($bios.ReleaseDate) { $bios.ReleaseDate.ToString('yyyy-MM-dd') } else { $null }
    firmware       = $fw
}

# ---- Network ----
$ipByIndex = @{}
foreach ($ipa in Get-NetIPAddress -AddressFamily IPv4) { if (-not $ipByIndex.ContainsKey($ipa.InterfaceIndex)) { $ipByIndex[$ipa.InterfaceIndex] = $ipa.IPAddress } }
$net = @(foreach ($a in Get-NetAdapter -Physical | Where-Object { $_.Status -ne 'Not Present' } | Sort-Object Status, Name) {
    $ip = $ipByIndex[$a.ifIndex]
    [ordered]@{
        name        = [string]$a.Name
        description = [string]$a.InterfaceDescription
        status      = [string]$a.Status
        linkSpeed   = [string]$a.LinkSpeed
        mac         = [string]$a.MacAddress
        mediaType   = [string]$a.PhysicalMediaType
        ipv4        = $ip
    }
})

# ---- Monitors ----
function Decode-Wmi($arr) { if ($arr) { (($arr | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join '').Trim() } else { $null } }
$params = @{}
foreach ($p in Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorBasicDisplayParams) { $params[$p.InstanceName] = $p }
$monitors = @(foreach ($m in Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID) {
    $p = $params[$m.InstanceName]
    $diag = $null
    if ($p -and $p.MaxHorizontalImageSize -gt 0) {
        $diag = [math]::Round([math]::Sqrt([math]::Pow($p.MaxHorizontalImageSize, 2) + [math]::Pow($p.MaxVerticalImageSize, 2)) / 2.54, 1)
    }
    [ordered]@{
        name         = Decode-Wmi $m.UserFriendlyName
        manufacturer = Decode-Wmi $m.ManufacturerName
        serial       = Decode-Wmi $m.SerialNumberID
        sizeInches   = $diag
        year         = [int]$m.YearOfManufacture
    }
})

# ---- Performance-relevant configuration ----
$plan = $null
$planOut = powercfg /getactivescheme
if ($planOut -match '([0-9a-fA-F-]{36})\s+\((.+)\)') { $plan = [ordered]@{ guid = $Matches[1]; name = $Matches[2] } }
$gameMode = (Get-ItemProperty 'HKCU:\Software\Microsoft\GameBar').AutoGameModeEnabled
$hags = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers').HwSchMode
$config = [ordered]@{
    powerPlan         = $plan
    gameModeEnabled   = if ($null -eq $gameMode) { $true } else { [bool]$gameMode }
    hagsEnabled       = if ($null -eq $hags) { $null } else { $hags -eq 2 }
}

[ordered]@{
    windows     = $windows
    cpu         = $cpu
    gpus        = $gpus
    memory      = $memory
    disks       = $disks
    board       = $board
    network     = $net
    monitors    = $monitors
    config      = $config
    collectedAt = (Get-Date).ToString('s')
} | ConvertTo-Json -Depth 6 -Compress
