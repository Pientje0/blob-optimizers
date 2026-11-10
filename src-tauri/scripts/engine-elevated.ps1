# Runs inside an elevated PowerShell started through UAC. The elevated process cannot share pipes with
# the (non-elevated) app, so the real command is started as a child whose output goes to files the app
# tails live. The app requests cancellation by creating the cancel file.
$ErrorActionPreference = 'Stop'
$out = '__OUT__'
$err = '__ERR__'
$cancel = '__CANCEL__'
try {
    $p = Start-Process -FilePath '__PROGRAM__' -ArgumentList '__ARGS__' -RedirectStandardOutput $out -RedirectStandardError $err -WindowStyle Hidden -PassThru
} catch {
    [IO.File]::AppendAllText($err, $_.Exception.Message)
    exit 9001
}
$null = $p.Handle # caching the handle keeps ExitCode available after exit
while (-not $p.HasExited) {
    if (Test-Path -LiteralPath $cancel) {
        & taskkill.exe /PID $p.Id __TREE__ /F | Out-Null
        exit 1223
    }
    Start-Sleep -Milliseconds 150
}
$p.WaitForExit()
exit $p.ExitCode
