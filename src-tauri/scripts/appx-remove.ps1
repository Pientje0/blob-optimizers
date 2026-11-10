# Blob Optimizers - remove Store (Appx) packages for the current user.
# Removal is per user: Windows keeps preinstalled packages available so they can be restored.
$packages = @(__PACKAGES__)
$failed = 0
$i = 0
foreach ($pkg in $packages) {
    $i++
    "##PROGRESS $([int](($i - 1) / $packages.Count * 100)) Removing $($pkg.name)"
    $installed = Get-AppxPackage -Name $pkg.package | Where-Object PackageFullName -eq $pkg.fullName
    if (-not $installed) { "$($pkg.name): already removed"; continue }
    try {
        Remove-AppxPackage -Package $pkg.fullName -ErrorAction Stop
        "$($pkg.name): removed"
    } catch {
        $failed++
        [Console]::Error.WriteLine("$($pkg.name): could not be removed - $($_.Exception.Message)")
    }
}
'##PROGRESS 100 Done'
if ($failed) { exit 1 }
