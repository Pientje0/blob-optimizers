$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$global:LASTEXITCODE = 0
$__blobFailed = $false
# Every stream is merged and rendered as plain text: Windows PowerShell would otherwise serialise
# errors to stderr as CLIXML when its output is redirected.
try {
    # The script arrives base64-encoded and is parsed here, so syntax errors are reported as plain text.
    $__blobScript = [ScriptBlock]::Create([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__BLOB_SCRIPT_B64__')))
    & $__blobScript *>&1 | ForEach-Object {
        if ($_ -is [System.Management.Automation.ErrorRecord]) {
            if ($_.FullyQualifiedErrorId -notlike 'NativeCommandError*') { $__blobFailed = $true }
            [Console]::Error.WriteLine($_.ToString())
        } elseif ($_ -is [System.Management.Automation.WarningRecord]) {
            "WARNING: $($_.Message)"
        } else { $_ }
    } | Out-String -Stream -Width 220
} catch {
    $__blobFailed = $true
    [Console]::Error.WriteLine($_.ToString())
}
if ($global:LASTEXITCODE) { exit $global:LASTEXITCODE }
if ($__blobFailed) { exit 1 }
exit 0
