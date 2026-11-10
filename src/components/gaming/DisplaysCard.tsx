import { Loader2, Monitor, MonitorCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useJob } from "../../hooks/useJob";
import { api, type DisplayInfo } from "../../lib/api";
import { Badge, Card, Skeleton } from "../ui/Card";
import { Modal } from "../ui/Modal";

const KEEP_SECONDS = 15;

export function DisplaysCard({ displays, reload }: { displays: DisplayInfo[] | null; reload: () => Promise<void> }) {
  const { run, busy } = useJob();
  const [choice, setChoice] = useState<Record<string, number>>({});
  const [confirm, setConfirm] = useState<{ d: DisplayInfo; old: number; hz: number } | null>(null);
  const [left, setLeft] = useState(KEEP_SECONDS);
  const timer = useRef<number>();

  const change = async (d: DisplayInfo, hz: number) => {
    const done = await run(d.device, () => api.setRefreshRate(d.device, hz), { failure: `Could not change ${d.monitor}` });
    await reload();
    if (done?.status === "succeeded") {
      setLeft(KEEP_SECONDS);
      setConfirm({ d, old: d.hz, hz });
    }
  };

  // Like Windows: revert automatically unless the user confirms the picture is fine.
  useEffect(() => {
    if (!confirm) return;
    timer.current = window.setInterval(() => setLeft((l) => l - 1), 1000);
    return () => window.clearInterval(timer.current);
  }, [confirm]);

  useEffect(() => {
    // Only the countdown reaching zero should trigger this.
    if (confirm && left <= 0) revert();
  }, [left]);

  const revert = async () => {
    if (!confirm) return;
    const c = confirm;
    setConfirm(null);
    await run(c.d.device, () => api.setRefreshRate(c.d.device, c.old), { success: `Reverted to ${c.old} Hz`, failure: "Could not revert" });
    await reload();
  };

  return (
    <Card title="Displays" icon={Monitor} className="h-full">
      {!displays ? (
        <Skeleton className="h-24 w-full" />
      ) : (
        <div className="space-y-3">
          {displays.map((d) => {
            const max = Math.max(...d.rates, d.hz);
            const selected = choice[d.device] ?? max;
            return (
              <div key={d.device} className="rounded-xl bg-raised/60 p-3">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-[13px] font-medium">{d.primary ? "Main display" : "Display"}</span>
                  <span className="text-xs text-muted">
                    {d.width}×{d.height} · {d.adapter}
                  </span>
                  {d.primary && <Badge tone="accent">Primary</Badge>}
                </div>
                <div className="mt-2 flex flex-wrap items-center gap-3">
                  <span className={`text-2xl font-semibold tabular-nums ${d.hz < max ? "text-warn" : "text-good"}`}>{d.hz} Hz</span>
                  {d.hz < max ? (
                    <span className="text-xs text-warn">Supports up to {max} Hz at this resolution</span>
                  ) : (
                    <span className="flex items-center gap-1 text-xs text-good">
                      <MonitorCheck size={13} /> Running at its highest refresh rate
                    </span>
                  )}
                  <span className="flex-1" />
                  <select
                    className="rounded-lg border border-line bg-base/60 px-2 py-1 text-xs outline-none focus:border-accent/60"
                    value={selected}
                    aria-label={`Refresh rate for ${d.device}`}
                    onChange={(e) => setChoice({ ...choice, [d.device]: +e.target.value })}
                  >
                    {d.rates.map((r) => (
                      <option key={r} value={r}>
                        {r} Hz
                      </option>
                    ))}
                  </select>
                  <button className="btn-primary px-3 py-1 text-xs" disabled={!!busy || selected === d.hz} onClick={() => change(d, selected)}>
                    {busy === d.device && <Loader2 size={13} className="animate-spin" />} Apply
                  </button>
                </div>
              </div>
            );
          })}
          <p className="text-xs text-muted">
            Only rates the monitor reports for its current resolution are offered. A higher refresh rate is usually the most visible smoothness upgrade; make
            sure the cable (DisplayPort or HDMI 2.0+) supports it.
          </p>
        </div>
      )}

      <Modal
        open={!!confirm}
        title="Keep this refresh rate?"
        onClose={revert}
        footer={
          <>
            <button className="btn-ghost" onClick={revert}>
              Revert ({left})
            </button>
            <button
              className="btn-primary"
              onClick={() => {
                setConfirm(null);
              }}
            >
              Keep {confirm?.hz} Hz
            </button>
          </>
        }
      >
        The display now runs at {confirm?.hz} Hz. If the picture is wrong or black, wait: it returns to {confirm?.old} Hz automatically in {left} seconds.
      </Modal>
    </Card>
  );
}
