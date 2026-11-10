import { AlertTriangle, ChevronDown, Loader2, RotateCcw, ShieldCheck, Undo2 } from "lucide-react";
import { useState } from "react";
import type { TweakInfo, TweakTarget } from "../../lib/api";
import { Badge } from "../ui/Card";
import { Switch } from "../ui/Controls";

const RECOMMEND = {
  apply: { label: "Recommended", tone: "good" },
  keep: { label: "Keep Windows default", tone: "warn" },
  either: { label: "Your choice", tone: "neutral" },
} as const;

const AFTER = {
  nothing: null,
  explorer: "Explorer restart",
  signOut: "Sign out to apply",
  restart: "Restart to apply",
} as const;

function stateLabel(t: TweakInfo): { label: string; cls: string } {
  if (t.atDefault) return { label: t.defaultOn ? "On (Windows default)" : "Windows default", cls: "text-muted" };
  if (t.state === "on") return { label: "Applied", cls: "text-good" };
  if (t.state === "off") return { label: "Off", cls: "text-warn" };
  return { label: "Custom", cls: "text-warn" };
}

interface Props {
  tweak: TweakInfo;
  busy: boolean;
  disabled: boolean;
  onChange: (t: TweakInfo, target: TweakTarget) => void;
}

export function TweakRow({ tweak: t, busy, disabled, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const after = AFTER[t.after];
  const state = stateLabel(t);

  return (
    <div className="px-5 py-4">
      <div className="flex items-start gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-medium">{t.title}</span>
            <Badge tone={RECOMMEND[t.recommend].tone}>{RECOMMEND[t.recommend].label}</Badge>
            {t.requiresAdmin && (
              <Badge tone="accent">
                <ShieldCheck size={10} /> Admin
              </Badge>
            )}
            {after && <Badge>{after}</Badge>}
          </div>
          <p className="mt-1 text-[13px] leading-relaxed text-muted">{t.description}</p>
          {t.caveat && (
            <p className="mt-1.5 flex gap-1.5 text-xs leading-relaxed text-warn/90">
              <AlertTriangle size={13} className="mt-px shrink-0" /> {t.caveat}
            </p>
          )}
          <div className="mt-2 flex flex-wrap items-center gap-3 text-xs">
            <span className={state.cls} title={t.state === "custom" ? "Set to something else, for example in Windows Settings or by a policy." : undefined}>
              Current: {state.label}
            </span>
            {!t.atDefault && (
              <button className="flex items-center gap-1 text-muted hover:text-fg disabled:opacity-40" disabled={disabled} onClick={() => onChange(t, "default")}>
                <RotateCcw size={12} /> Reset to default
              </button>
            )}
            {t.canUndo && (
              <button
                className="flex items-center gap-1 text-muted hover:text-fg disabled:opacity-40"
                disabled={disabled}
                onClick={() => onChange(t, "undo")}
                title="Put back exactly what was set before the last change made here"
              >
                <Undo2 size={12} /> Undo last change
              </button>
            )}
            <button className="flex items-center gap-1 text-muted hover:text-fg" onClick={() => setOpen(!open)}>
              <ChevronDown size={12} className={`transition-transform ${open ? "rotate-180" : ""}`} /> What changes
            </button>
          </div>
          {open && (
            <div className="selectable mt-2 grid gap-2 rounded-xl bg-base/60 p-3 font-mono text-[11.5px] leading-relaxed sm:grid-cols-2">
              <div>
                <div className="mb-1 font-sans text-[10.5px] font-semibold uppercase tracking-wider text-good">{t.defaultOn ? "On (Windows default)" : "Applied"}</div>
                {t.changesOn.map((c) => (
                  <div key={c} className="break-all">
                    {c}
                  </div>
                ))}
              </div>
              <div>
                <div className="mb-1 font-sans text-[10.5px] font-semibold uppercase tracking-wider text-muted">{t.defaultOn ? "Off" : "Windows default"}</div>
                {t.changesOff.map((c) => (
                  <div key={c} className="break-all">
                    {c}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
        <div className="flex h-7 items-center">
          {busy ? (
            <Loader2 size={18} className="mx-2.5 animate-spin text-accent" />
          ) : (
            <Switch label={t.title} checked={t.state === "on"} disabled={disabled} onChange={(v) => onChange(t, v ? "on" : t.defaultOn ? "off" : "default")} />
          )}
        </div>
      </div>
    </div>
  );
}
