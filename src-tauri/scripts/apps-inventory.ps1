# Blob Optimizers - read-only inventory of installed desktop programs and Store (Appx) apps.
$ErrorActionPreference = 'SilentlyContinue'

$roots = @(
    @{ hive = 'HKLM64'; path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' },
    @{ hive = 'HKLM32'; path = 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall' },
    @{ hive = 'HKCU'; path = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall' }
)

$win32 = @(foreach ($r in $roots) {
    foreach ($k in Get-ChildItem -LiteralPath $r.path) {
        $p = Get-ItemProperty -LiteralPath $k.PSPath
        $name = ([string]$p.DisplayName).Trim()
        # Hidden components, updates/hotfixes and child entries are not separate programs.
        if (-not $name -or $p.SystemComponent -eq 1 -or $p.ParentKeyName -or $p.ReleaseType -match 'Update|Hotfix') { continue }
        if (-not $p.UninstallString -and -not $p.QuietUninstallString) { continue }
        [ordered]@{
            hive            = $r.hive
            key             = $k.PSChildName
            name            = $name
            publisher       = ([string]$p.Publisher).Trim()
            version         = [string]$p.DisplayVersion
            installDate     = [string]$p.InstallDate
            sizeKb          = if ($p.EstimatedSize) { [uint64]$p.EstimatedSize } else { $null }
            installLocation = ([string]$p.InstallLocation).Trim().Trim('"')
            uninstallString = [string]$p.UninstallString
            quiet           = [bool]$p.QuietUninstallString
            msi             = ($p.WindowsInstaller -eq 1)
            noRemove        = ($p.NoRemove -eq 1)
        }
    }
})

# Friendly names for Store apps come from the Start menu (resolves ms-resource: strings for us).
$startNames = @{}
foreach ($s in Get-StartApps) {
    if ($s.AppID -match '^([^!]+)!') { if (-not $startNames.ContainsKey($Matches[1])) { $startNames[$Matches[1]] = $s.Name } }
}

$appx = @(foreach ($a in Get-AppxPackage) {
    if ($a.IsFramework -or $a.IsResourcePackage) { continue }
    $date = $null
    if ($a.InstallLocation) { $item = Get-Item -LiteralPath $a.InstallLocation; if ($item) { $date = $item.CreationTime.ToString('yyyyMMdd') } }
    [ordered]@{
        name            = [string]$a.Name
        fullName        = [string]$a.PackageFullName
        family          = [string]$a.PackageFamilyName
        displayName     = $startNames[[string]$a.PackageFamilyName]
        publisher       = ([string]$a.Publisher -replace '^CN=("?)([^,"]+).*$', '$2')
        version         = [string]$a.Version
        signatureKind   = [string]$a.SignatureKind
        nonRemovable    = [bool]$a.NonRemovable
        installLocation = [string]$a.InstallLocation
        installDate     = $date
    }
})

[ordered]@{ win32 = $win32; appx = $appx } | ConvertTo-Json -Depth 4 -Compress
