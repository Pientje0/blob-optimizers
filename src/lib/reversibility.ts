/** Operations whose effects cannot be undone automatically by Blob Optimizers. */
const IRREVERSIBLE: Record<string, string> = {
  "apps.uninstall": "Uninstalled programs must be reinstalled from their installer (a restore point can roll the system back).",
  "system.cleanup": "Deleted temporary files and emptied Recycle Bin items cannot be recovered.",
  "repair.dism_restore": "Repaired Windows components are replaced with Microsoft's originals.",
  "repair.sfc_scan": "Repaired system files are replaced with Microsoft's originals.",
  "power.restore_defaults": "Custom power plans that were removed are gone.",
  "net.reset_winsock": "Removed network layers (VPN/proxy software) must be repaired or reinstalled.",
  "net.reset_tcpip": "Manually set IP and DNS settings were cleared.",
  "restore.rollback": "Rolling back a restore point is itself recorded as a restore point by Windows and can be undone from there.",
};

export function irreversibleReason(operationId: string | null | undefined): string | null {
  return operationId ? IRREVERSIBLE[operationId] ?? null : null;
}
