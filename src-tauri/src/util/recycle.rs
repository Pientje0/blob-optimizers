/// PowerShell snippet defining `[BlobRecycle]::ToRecycleBin(path)`: deletes to the Recycle Bin without
/// confirmation or error dialogs. Returns 0 on success.
pub const PS_RECYCLE_BIN: &str = r#"if (-not ('BlobRecycle' -as [type])) {
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class BlobRecycle {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct SHFILEOPSTRUCT { public IntPtr hwnd; public uint wFunc; public string pFrom; public string pTo; public ushort fFlags; public bool fAnyOperationsAborted; public IntPtr hNameMappings; public string lpszProgressTitle; }
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)] static extern int SHFileOperation(ref SHFILEOPSTRUCT op);
    // FO_DELETE with FOF_ALLOWUNDO (Recycle Bin), no confirmation, no error UI, silent.
    public static int ToRecycleBin(string path) {
        var op = new SHFILEOPSTRUCT { wFunc = 3, pFrom = path + "\0\0", fFlags = 0x0040 | 0x0010 | 0x0400 | 0x0004 };
        return SHFileOperation(ref op);
    }
}
'@
}
"#;
