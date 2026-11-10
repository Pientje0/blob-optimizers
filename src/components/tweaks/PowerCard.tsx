import { BatteryCharging, Check, Loader2, Plus, RotateCcw, Trash2, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { useJob } from "../../hooks/useJob";
import { api, type SystemInfo } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { Badge, Card } from "../ui/Card";
import { Switch } from "../ui/Controls";
import { Modal } from "../ui/Modal";

const NUM = "w-20 rounded-lg border border-line bg-base/60 px-2 py-1 text-right text-sm tabular-nums outline-none focus:border-accent/60";

export function PowerCard({ info, reload }: { info: SystemInfo; reload: () => void }) {
  const { run, busy } = useJob();
  const [t, setT] = useState({ monitorAc: 0, sleepAc: 0, monitorDc: 0, sleepDc: 0 });
  const [confirmReset, setConfirmReset] = useState(false);

  useEffect(() => {
    setT({ monitorAc: info.monitorAc ?? 0, sleepAc: info.sleepAc ?? 0, monitorDc: info.monitorDc ?? 0, sleepDc: info.sleepDc ?? 0 });
  }, [info]);

  const after = async (p: Promise<unknown>) => {
    await p;
    reload();
  };
  const changed =
    t.monitorAc !== (info.monitorAc ?? 0) || t.sleepAc !== (info.sleepAc ?? 0) || (info.hasBattery && (t.monitorDc !== (info.monitorDc ?? 0) || t.sleepDc !== (info.sleepDc ?? 0)));
  const hasUltimate = info.plans.some((p) => /ultimate/i.test(p.name));
  const hasHigh = info.plans.some((p) => p.guid === "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" || /high performance/i.test(p.name));

  return (
    <Card title="Power" icon={Zap} className="h-full">
      <div className="label mb-2">Power plan</div>
      <div className="space-y-1.5">
        {info.plans.map((p) => (
          <div key={p.guid} className={`flex items-center gap-3 rounded-xl px-3 py-2.5 ${p.active ? "bg-accent/10" : "bg-raised/60"}`}>
            <button
              className="flex min-w-0 flex-1 items-center gap-2.5 text-left"
              disabled={p.active || !!busy}
              onClick={() => after(run(`plan-${p.guid}`, () => api.setPowerPlan(p.guid), { success: `Switched to ${p.name}`, failure: "Could not switch plan" }))}
            >
              <span className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${p.active ? "border-accent bg-accent" : "border-line"}`}>
                {p.active && <Check size={11} className="text-white" />}
              </span>
              <span className="truncate text-[13px] font-medium">{p.name}</span>
              {p.active && <Badge tone="accent">Active</Badge>}
            </button>
            {busy === `plan-${p.guid}` && <Loader2 size={14} className="animate-spin text-accent" />}
            {!p.active && !p.builtIn && (
              <button
                className="text-muted hover:text-bad"
                title="Delete this plan"
                disabled={!!busy}
                onClick={() => after(run(`del-${p.guid}`, () => api.deletePowerPlan(p.guid), { success: `Deleted ${p.name}`, failure: "Could not delete plan" }))}
              >
                <Trash2 size={14} />
              </button>
            )}
          </div>
        ))}
      </div>
      <div className="mt-2 flex flex-wrap gap-2">
        {!hasHigh && (
          <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!!busy} onClick={() => after(run("add-high", () => api.addPowerPlan("high"), { success: "High performance plan added", failure: "Could not add the plan" }))}>
            <Plus size={13} /> High performance
          </button>
        )}
        {!hasUltimate && (
          <button
            className="btn-ghost px-3 py-1.5 text-xs"
            disabled={!!busy}
            title="Keeps the CPU at full readiness; uses more power. Mainly useful on desktops."
            onClick={() => after(run("add-ultimate", () => api.addPowerPlan("ultimate"), { success: "Ultimate Performance plan added", failure: "Could not add the plan" }))}
          >
            <Plus size={13} /> Ultimate Performance
          </button>
        )}
        <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!!busy} onClick={() => setConfirmReset(true)}>
          <RotateCcw size={13} /> Restore default plans
        </button>
      </div>
      <p className="mt-2 text-xs text-muted">Higher-performance plans keep clocks up for snappier response at the cost of power and heat.</p>

      <div className="label mb-2 mt-5">Screen and sleep (active plan, minutes, 0 = never)</div>
      <div className="grid grid-cols-[1fr_auto_auto] items-center gap-x-4 gap-y-2 text-[13px]">
        <span />
        <span className="text-xs text-muted">Plugged in</span>
        <span className="text-xs text-muted">{info.hasBattery ? "On battery" : ""}</span>
        <span>Turn off screen after</span>
        <input type="number" min={0} max={600} className={NUM} value={t.monitorAc} onChange={(e) => setT({ ...t, monitorAc: +e.target.value })} />
        {info.hasBattery ? <input type="number" min={0} max={600} className={NUM} value={t.monitorDc} onChange={(e) => setT({ ...t, monitorDc: +e.target.value })} /> : <span />}
        <span>Sleep after</span>
        <input type="number" min={0} max={600} className={NUM} value={t.sleepAc} onChange={(e) => setT({ ...t, sleepAc: +e.target.value })} />
        {info.hasBattery ? <input type="number" min={0} max={600} className={NUM} value={t.sleepDc} onChange={(e) => setT({ ...t, sleepDc: +e.target.value })} /> : <span />}
      </div>
      <button
        className="btn-primary mt-3 px-3 py-1.5 text-xs"
        disabled={!changed || !!busy}
        onClick={() =>
          after(
            run("timeouts", () => api.setTimeouts({ monitorAc: t.monitorAc, sleepAc: t.sleepAc, monitorDc: info.hasBattery ? t.monitorDc : null, sleepDc: info.hasBattery ? t.sleepDc : null }), {
              success: "Timeouts saved",
              failure: "Could not change timeouts",
            }),
          )
        }
      >
        Save timeouts
      </button>

      <div className="mt-5 flex items-center gap-4 rounded-xl bg-raised/60 px-3 py-3">
        <BatteryCharging size={16} className="text-accent" />
        <div className="min-w-0 flex-1 text-[13px]">
          <div className="font-medium">Hibernation</div>
          <div className="text-xs text-muted">
            Needed for Fast Startup and hibernate.{" "}
            {info.hibernate && info.hiberfilBytes ? `Turning it off frees ${formatBytes(info.hiberfilBytes)} (hiberfil.sys).` : ""}
          </div>
        </div>
        {busy === "hib" ? (
          <Loader2 size={16} className="animate-spin text-accent" />
        ) : (
          <Switch
            label="Hibernation"
            checked={info.hibernate}
            disabled={!!busy}
            onChange={(v) => after(run("hib", () => api.setHibernate(v), { success: v ? "Hibernation on" : "Hibernation off", failure: "Could not change hibernation" }))}
          />
        )}
      </div>

      <Modal
        open={confirmReset}
        title="Restore default power plans?"
        onClose={() => setConfirmReset(false)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirmReset(false)}>
              Cancel
            </button>
            <button
              className="btn-primary"
              onClick={() => {
                setConfirmReset(false);
                after(run("reset", () => api.restoreDefaultPlans(), { success: "Power plans restored", failure: "Could not restore plans" }));
              }}
            >
              Restore defaults
            </button>
          </>
        }
      >
        Resets all power plans and their settings to Windows defaults. Custom plans (including any added here) are deleted. This cannot be undone.
      </Modal>
    </Card>
  );
}
