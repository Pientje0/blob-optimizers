import { Lock } from "lucide-react";
import type { InstalledApp, Recommendation } from "../../lib/api";
import { Badge } from "../ui/Card";

export const REC_META: Record<Recommendation, { label: string; tone: "good" | "neutral" | "warn" | "accent" }> = {
  safe: { label: "Safe to remove", tone: "good" },
  optional: { label: "Your choice", tone: "neutral" },
  caution: { label: "Caution", tone: "warn" },
  protected: { label: "Protected", tone: "accent" },
};

export function RecommendationBadge({ rec }: { rec: Recommendation }) {
  const m = REC_META[rec];
  return (
    <Badge tone={m.tone}>
      {rec === "protected" && <Lock size={10} />}
      {m.label}
    </Badge>
  );
}

const KIND_LABEL = { runtime: "Runtime", driver: "Driver", system: "System", app: null } as const;

export function AppTypeBadges({ app }: { app: InstalledApp }) {
  const kind = KIND_LABEL[app.appKind];
  return (
    <>
      <Badge>{app.kind === "appx" ? "Store app" : "Desktop"}</Badge>
      {kind && <Badge>{kind}</Badge>}
      {app.preinstalled && <Badge>Preinstalled</Badge>}
    </>
  );
}
