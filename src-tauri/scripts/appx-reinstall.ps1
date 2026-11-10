# Blob Optimizers - restore a Store app that was removed for this user.
# 1) Re-register the copy Windows keeps for preinstalled apps (offline, instant).
# 2) Otherwise install it again from the Microsoft Store through winget.
$package = '__PACKAGE__'
$name = '__NAME__'
$storeId = '__STOREID__'
$location = '__LOCATION__'

if (Get-AppxPackage -Name $package) { "$name is already installed."; exit 0 }

'##PROGRESS 10 Looking for the copy Windows keeps'
$manifest = if ($location) { Join-Path $location 'AppxManifest.xml' } else { $null }
if ($manifest -and (Test-Path -LiteralPath $manifest)) {
    try {
        Add-AppxPackage -DisableDevelopmentMode -Register $manifest -ErrorAction Stop
        "$name was restored from the copy kept by Windows."
        '##PROGRESS 100 Restored'
        exit 0
    } catch {
        "Re-registering the local copy failed: $($_.Exception.Message)"
    }
} else {
    'Windows no longer keeps a local copy of this app.'
}

if (-not $storeId) {
    [Console]::Error.WriteLine("This app has no known Store ID, so it cannot be downloaded automatically. Install $name from the Microsoft Store.")
    exit 1
}
if (-not (Get-Command winget.exe -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('winget (App Installer) is not available, so the app cannot be downloaded automatically.')
    exit 1
}
'##PROGRESS 30 Downloading from the Microsoft Store'
"Installing $name ($storeId) from the Microsoft Store. This accepts the Store's package and source agreements."
& winget.exe install --id $storeId --source msstore --exact --silent --accept-package-agreements --accept-source-agreements --disable-interactivity
if (Get-AppxPackage -Name $package) { "$name was reinstalled."; exit 0 }
exit $(if ($LASTEXITCODE) { $LASTEXITCODE } else { 1 })
