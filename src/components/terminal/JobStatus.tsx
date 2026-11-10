import { Ban, CheckCircle2, Clock, Loader2, ShieldX, XCircle } from "lucide-react";
import type { HistoryEntry, JobStatus } from "../../lib/api";
import { Badge } from "../ui/Card";

const META: Record<JobStatus, { label: string; tone: "good" | "warn" | "bad" | "neutral" | "accent"; icon: JSX.Element }> = {
  running: { label: "Running", tone: "accent", icon: <Loader2 size={12} className="animate-spin" /> },
  succeeded: { label: "Succeeded", tone: "good", icon: <CheckCircle2 size={12} /> },
  failed: { label: "Failed", tone: "bad", icon: <XCircle size={12} /> },
  cancelled: { label: "Cancelled", tone: "neutral", icon: <Ban size={12} /> },
  timedOut: { label: "Timed out", tone: "warn", icon: <Clock size={12} /> },
  declined: { label: "Permission declined", tone: "warn", icon: <ShieldX size={12} /> },
};

export function JobStatusBadge({ status }: { status: JobStatus }) {
  const m = META[status];
  return (
    <Badge tone={m.tone}>
      {m.icon}
      {m.label}
    </Badge>
  );
}

export function statusIcon(status: JobStatus) {
  return META[status].icon;
}

export function jobDuration(e: HistoryEntry, now = Date.now()) {
  return (e.finishedAt ?? now) - e.startedAt;
}
