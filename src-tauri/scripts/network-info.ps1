# Blob Optimizers - read-only network configuration.
$ErrorActionPreference = 'SilentlyContinue'

$wifi = @{}
$cur = $null
foreach ($line in (netsh wlan show interfaces)) {
    if ($line -match '^\s*Name\s*:\s*(.+)$') { $cur = $Matches[1].Trim(); $wifi[$cur] = @{} }
    elseif ($cur -and $line -match '^\s*SSID\s*:\s*(.+)$') { $wifi[$cur].ssid = $Matches[1].Trim() }
    elseif ($cur -and $line -match '^\s*Signal\s*:\s*(\d+)\s*%') { $wifi[$cur].signal = [int]$Matches[1] }
    elseif ($cur -and $line -match '^\s*Radio type\s*:\s*(.+)$') { $wifi[$cur].radio = $Matches[1].Trim() }
    elseif ($cur -and $line -match '^\s*Channel\s*:\s*(\d+)') { $wifi[$cur].channel = [int]$Matches[1] }
    elseif ($cur -and $line -match '^\s*Band\s*:\s*(.+)$') { $wifi[$cur].band = $Matches[1].Trim() }
}

# Skip hidden adapters and ghosts of hardware that is no longer installed ("Not Present").
$adapters = @(foreach ($a in Get-NetAdapter | Where-Object { -not $_.Hidden -and $_.Status -and $_.Status -ne 'Not Present' } | Sort-Object Status, Name) {
    $cfg = Get-NetIPConfiguration -InterfaceIndex $a.ifIndex
    $ipif = Get-NetIPInterface -InterfaceIndex $a.ifIndex -AddressFamily IPv4
    $dns4 = @((Get-DnsClientServerAddress -InterfaceIndex $a.ifIndex -AddressFamily IPv4).ServerAddresses)
    $dns6 = @((Get-DnsClientServerAddress -InterfaceIndex $a.ifIndex -AddressFamily IPv6).ServerAddresses)
    $profile = Get-NetConnectionProfile -InterfaceIndex $a.ifIndex
    # DNS servers set by hand are stored in NameServer; DHCP-provided ones in DhcpNameServer.
    $manualDns = [string](Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$($a.InterfaceGuid)").NameServer
    $w = $wifi[$a.Name]
    [ordered]@{
        index       = [int]$a.ifIndex
        name        = [string]$a.Name
        description = [string]$a.InterfaceDescription
        status      = [string]$a.Status
        mediaType   = [string]$a.PhysicalMediaType
        physical    = [bool]$a.HardwareInterface
        linkSpeed   = [string]$a.LinkSpeed
        mac         = [string]$a.MacAddress
        ipv4        = @($cfg.IPv4Address | Where-Object { $_.IPAddress } | ForEach-Object { "$($_.IPAddress)/$($_.PrefixLength)" })
        ipv6        = @($cfg.IPv6Address | Where-Object { $_.IPAddress } | ForEach-Object { [string]$_.IPAddress })
        gateway     = [string]($cfg.IPv4DefaultGateway | Select-Object -First 1).NextHop
        dhcp        = ([string]$ipif.Dhcp -eq 'Enabled')
        mtu         = [int]$ipif.NlMtu
        dns         = @(($dns4 + $dns6) | Where-Object { $_ })
        dnsManual   = [bool]$manualDns
        category    = if ($profile) { [string]$profile.NetworkCategory } else { $null }
        networkName = if ($profile) { [string]$profile.Name } else { $null }
        ssid        = if ($w) { $w.ssid } else { $null }
        signal      = if ($w) { $w.signal } else { $null }
        radio       = if ($w) { $w.radio } else { $null }
        band        = if ($w) { $w.band } else { $null }
        channel     = if ($w) { $w.channel } else { $null }
    }
})
ConvertTo-Json -InputObject $adapters -Depth 4 -Compress
