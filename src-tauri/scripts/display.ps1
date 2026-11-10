# Blob Optimizers - display modes. Mode 'list' reads; mode 'set' changes one display's refresh rate.
$mode = '__MODE__'
$target = '__DEVICE__'
$hz = [int]'__HZ__'
$ErrorActionPreference = 'Stop'
if (-not ('BlobDisplay' -as [type])) {
Add-Type -TypeDefinition @'
using System; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class BlobDisplay {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DISPLAY_DEVICE {
        public int cb;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceString;
        public int StateFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceID;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceKey;
    }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODE {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra;
        public int dmFields, dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
        public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public short dmLogPixels;
        public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
        public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool EnumDisplayDevices(string dev, int i, ref DISPLAY_DEVICE dd, int flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool EnumDisplaySettings(string dev, int mode, ref DEVMODE dm);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int ChangeDisplaySettingsEx(string dev, ref DEVMODE dm, IntPtr hwnd, int flags, IntPtr param);
    public static DEVMODE Make() { var d = new DEVMODE(); d.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE)); return d; }
}
'@
}

function Current($dev) { $dm = [BlobDisplay]::Make(); [void][BlobDisplay]::EnumDisplaySettings($dev, -1, [ref]$dm); $dm }

if ($mode -eq 'list') {
    $out = @()
    for ($i = 0; $i -lt 16; $i++) {
        $dd = New-Object BlobDisplay+DISPLAY_DEVICE; $dd.cb = [Runtime.InteropServices.Marshal]::SizeOf($dd)
        # [NullString]::Value: PowerShell would otherwise pass $null to a string parameter as "".
        if (-not [BlobDisplay]::EnumDisplayDevices([NullString]::Value, $i, [ref]$dd, 0)) { break }
        if (($dd.StateFlags -band 1) -eq 0) { continue } # not attached to the desktop
        $mon = New-Object BlobDisplay+DISPLAY_DEVICE; $mon.cb = [Runtime.InteropServices.Marshal]::SizeOf($mon)
        [void][BlobDisplay]::EnumDisplayDevices($dd.DeviceName, 0, [ref]$mon, 0)
        $cur = Current $dd.DeviceName
        $rates = New-Object System.Collections.Generic.SortedSet[int]
        $j = 0; $dm = [BlobDisplay]::Make()
        while ([BlobDisplay]::EnumDisplaySettings($dd.DeviceName, $j, [ref]$dm)) {
            if ($dm.dmPelsWidth -eq $cur.dmPelsWidth -and $dm.dmPelsHeight -eq $cur.dmPelsHeight -and $dm.dmBitsPerPel -eq $cur.dmBitsPerPel -and $dm.dmDisplayFrequency -gt 1) { [void]$rates.Add($dm.dmDisplayFrequency) }
            $j++; $dm = [BlobDisplay]::Make()
        }
        $out += [ordered]@{
            device  = $dd.DeviceName
            adapter = $dd.DeviceString
            monitor = $mon.DeviceString
            primary = (($dd.StateFlags -band 4) -ne 0)
            width   = $cur.dmPelsWidth
            height  = $cur.dmPelsHeight
            hz      = $cur.dmDisplayFrequency
            rates   = @($rates)
        }
    }
    ConvertTo-Json -InputObject @($out) -Depth 3 -Compress
    exit 0
}

# set: same resolution, new refresh rate. CDS_UPDATEREGISTRY so it persists.
$dm = Current $target
$old = $dm.dmDisplayFrequency
$dm.dmDisplayFrequency = $hz
$dm.dmFields = 0x400000 # DM_DISPLAYFREQUENCY
$r = [BlobDisplay]::ChangeDisplaySettingsEx($target, [ref]$dm, [IntPtr]::Zero, 0x01, [IntPtr]::Zero)
if ($r -ne 0) { [Console]::Error.WriteLine("Windows rejected the change (code $r). The display stays at $old Hz."); exit 1 }
"$target now runs at $hz Hz (was $old Hz)."
