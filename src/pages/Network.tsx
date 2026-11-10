import {
  ArrowDown,
  ArrowUp,
  Cable,
  CheckCircle2,
  Gauge,
  Globe,
  Loader2,
  Power,
  RefreshCw,
  Stethoscope,
  Wifi,
  Wrench,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { NetworkChart } from "../components/charts/NetworkChart";
import { Badge, Card, InfoRow, Skeleton } from "../components/ui/Card";
import { Modal } from "../components/ui/Modal";
import { OperationButton } from "../components/ui/OperationButton";
import { useSystem } from "../context/SystemContext";
import { useToast } from "../context/ToastContext";
import { useJob } from "../hooks/useJob";
import { api, errorMessage, type DnsBench, type NetAdapter, type NetCheck } from "../lib/api";
import { formatBytes, formatRate } from "../lib/format";

const IP_RE = /^(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)$|^[0-9a-fA-F:]{2,39}$/;

function DnsDialog({ adapter, presets, initial, onClose, onDone }: { adapter: NetAdapter; presets: [string, string, string][]; initial?: string[]; onClose: () => void; onDone: () => void }) {
  const { run } = useJob();
  const [mode, setMode] = useState<"auto" | "preset" | "custom">(initial ? "preset" : adapter.dnsManual ? "custom" : "auto");
  const [preset, setPreset] = useState(initial ? presets.find((p) => p[1] === initial[0])?.[0] ?? presets[0][0] : presets[0][0]);
  const [custom, setCustom] = useState(adapter.dnsManual ? adapter.dns.filter((d) => d.includes(".")).slice(0, 2).join(", ") : "");
  const servers =
    mode === "auto" ? [] : mode === "preset" ? (presets.find((p) => p[0] === preset)?.slice(1) as string[]) : custom.split(/[\s,;]+/).filter(Boolean);
  const invalid = servers.find((s) => !IP_RE.test(s));

  const apply = async () => {
    onClose();
    await run(`dns-${adapter.index}`, () => api.setDns(adapter.index, servers), {
      success: `DNS updated on ${adapter.name}`,
      failure: `Could not change DNS on ${adapter.name}`,
    });
    onDone();
  };

  return (
    <Modal
      open
      title={`DNS servers for ${adapter.name}`}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn-primary" disabled={!!invalid || (mode === "custom" && !servers.length)} onClick={apply}>
            Apply
          </button>
        </>
      }
    >
      <div className="space-y-3 text-[13px]">
        <p className="text-muted">Current: {adapter.dns.join(", ") || "none"} ({adapter.dnsManual ? "set manually" : "automatic from router"})</p>
        <label className="flex cursor-pointer items-center gap-2.5">
          <input type="radio" checked={mode === "auto"} onChange={() => setMode("auto")} className="accent-[rgb(var(--c-accent))]" /> Automatic (from your router)
        </label>
        <label className="flex cursor-pointer items-center gap-2.5">
          <input type="radio" checked={mode === "preset"} onChange={() => setMode("preset")} className="accent-[rgb(var(--c-accent))]" /> Public DNS
          <select
            value={preset}
            onChange={(e) => {
              setPreset(e.target.value);
              setMode("preset");
            }}
            className="rounded-lg border border-line bg-base/60 px-2 py-1 text-xs outline-none"
          >
            {presets.map(([name, a, b]) => (
              <option key={name} value={name}>
                {name} ({a}, {b})
              </option>
            ))}
          </select>
        </label>
        <label className="flex cursor-pointer items-center gap-2.5">
          <input type="radio" checked={mode === "custom"} onChange={() => setMode("custom")} className="accent-[rgb(var(--c-accent))]" /> Custom
          <input
            value={custom}
            onFocus={() => setMode("custom")}
            onChange={(e) => setCustom(e.target.value)}
            placeholder="e.g. 1.1.1.1, 1.0.0.1"
            className="flex-1 rounded-lg border border-line bg-base/60 px-2 py-1 font-mono text-xs outline-none focus:border-accent/60"
          />
        </label>
        {invalid && <p className="text-xs text-bad">"{invalid}" is not a valid IP address.</p>}
        <p className="text-xs text-muted">
          Changes only this adapter, flushes the DNS cache and takes effect immediately. Public DNS can be faster or filter malware (Quad9, AdGuard); it can also bypass
          parental controls set on your router. Switch back to Automatic at any time.
        </p>
      </div>
    </Modal>
  );
}

export function Network() {
  const { live, history } = useSystem();
  const toast = useToast();
  const { run, busy } = useJob();
  const [adapters, setAdapters] = useState<NetAdapter[] | null>(null);
  const [presets, setPresets] = useState<[string, string, string][]>([]);
  const [checks, setChecks] = useState<NetCheck[] | null>(null);
  const [diagnosing, setDiagnosing] = useState(false);
  const [bench, setBench] = useState<DnsBench[] | null>(null);
  const [benching, setBenching] = useState(false);
  const [dnsFor, setDnsFor] = useState<{ a: NetAdapter; initial?: string[] } | null>(null);
  const [confirm, setConfirm] = useState<{ a: NetAdapter; action: "disable" | "enable" | "private" | "public" } | null>(null);

  const load = useCallback(async () => {
    try {
      setAdapters(await api.netAdapters());
    } catch (e) {
      toast("error", "Could not read network adapters", errorMessage(e));
    }
  }, [toast]);

  useEffect(() => {
    load();
    api.dnsPresets().then(setPresets).catch(() => undefined);
  }, [load]);

  const primary = useMemo(() => adapters?.find((a) => a.status === "Up" && a.gateway) ?? adapters?.find((a) => a.status === "Up"), [adapters]);

  const diagnose = async () => {
    setDiagnosing(true);
    try {
      setChecks(await api.netDiagnose());
    } catch (e) {
      toast("error", "Diagnostics failed", errorMessage(e));
    } finally {
      setDiagnosing(false);
    }
  };

  const benchmark = async () => {
    setBenching(true);
    try {
      setBench(await api.dnsBenchmark(primary?.dns ?? []));
    } catch (e) {
      toast("error", "DNS benchmark failed", errorMessage(e));
    } finally {
      setBenching(false);
    }
  };

  const doConfirm = async () => {
    if (!confirm) return;
    const { a, action } = confirm;
    setConfirm(null);
    const start =
      action === "enable" || action === "disable" ? () => api.setAdapterEnabled(a.index, action === "enable") : () => api.setNetworkCategory(a.index, action === "private");
    await run(`adapter-${a.index}`, start, { success: `${a.name} updated`, failure: `Could not change ${a.name}` });
    load();
  };

  const rateFor = (name: string) => live?.interfaces.find((i) => i.name === name);

  return (
    <div className="space-y-5">
      <Card
        title="Network activity"
        icon={Gauge}
        actions={
          <div className="flex gap-4 text-xs tabular-nums">
            <span className="flex items-center gap-1 text-accent2">
              <ArrowDown size={13} /> {live ? formatRate(live.netRxPerSec) : "…"}
            </span>
            <span className="flex items-center gap-1 text-accent">
              <ArrowUp size={13} /> {live ? formatRate(live.netTxPerSec) : "…"}
            </span>
          </div>
        }
      >
        <NetworkChart data={history} height={170} />
      </Card>

      <div className="flex items-center justify-between">
        <h2 className="text-[15px] font-semibold">Adapters</h2>
        <button className="btn-ghost px-3 py-1.5 text-xs" onClick={load}>
          <RefreshCw size={13} /> Refresh
        </button>
      </div>
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        {!adapters
          ? Array.from({ length: 2 }).map((_, i) => <Skeleton key={i} className="h-64" />)
          : adapters.map((a) => {
              const up = a.status === "Up";
              const rate = rateFor(a.name);
              const wifi = /802\.11/.test(a.mediaType);
              return (
                <Card
                  key={a.index}
                  title={
                    <span className="flex items-center gap-2">
                      {a.name}
                      <Badge tone={up ? "good" : a.status === "Disabled" ? "warn" : "neutral"}>{a.status}</Badge>
                      {a === primary && <Badge tone="accent">In use</Badge>}
                    </span>
                  }
                  icon={wifi ? Wifi : Cable}
                >
                  <div className="text-[13px]">
                    <InfoRow label="Adapter" value={a.description} />
                    {up && <InfoRow label="Link speed" value={a.linkSpeed} />}
                    {a.ssid && <InfoRow label="Wi-Fi" value={`${a.ssid}${a.signal != null ? ` · ${a.signal}% signal` : ""}${a.band ? ` · ${a.band}` : ""}${a.channel ? ` · ch ${a.channel}` : ""}`} />}
                    {a.ipv4.length > 0 && <InfoRow label="IPv4" value={a.ipv4.join(", ")} mono />}
                    {a.gateway && <InfoRow label="Gateway" value={a.gateway} mono />}
                    <InfoRow label="DNS" value={`${a.dns.slice(0, 3).join(", ") || "—"}${a.dns.length ? (a.dnsManual ? " (manual)" : " (automatic)") : ""}`} mono />
                    {a.category && <InfoRow label="Network type" value={a.category === "DomainAuthenticated" ? "Domain" : a.category} />}
                    {rate && up && <InfoRow label="Traffic now" value={`↓ ${formatRate(rate.rxPerSec)} · ↑ ${formatRate(rate.txPerSec)} (total ${formatBytes(rate.totalRx)} / ${formatBytes(rate.totalTx)})`} />}
                    <InfoRow label="MAC" value={a.mac || "—"} mono />
                  </div>
                  <div className="mt-4 flex flex-wrap gap-2">
                    {up && (
                      <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!!busy || !presets.length} onClick={() => setDnsFor({ a })}>
                        <Globe size={13} /> Change DNS
                      </button>
                    )}
                    {up && a.category && a.category !== "DomainAuthenticated" && (
                      <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!!busy} onClick={() => setConfirm({ a, action: a.category === "Public" ? "private" : "public" })}>
                        Make {a.category === "Public" ? "private" : "public"}
                      </button>
                    )}
                    <button
                      className="btn-ghost px-3 py-1.5 text-xs"
                      disabled={!!busy}
                      onClick={() => setConfirm({ a, action: a.status === "Disabled" ? "enable" : "disable" })}
                    >
                      {busy === `adapter-${a.index}` ? <Loader2 size={13} className="animate-spin" /> : <Power size={13} />}
                      {a.status === "Disabled" ? "Enable" : "Disable"}
                    </button>
                  </div>
                </Card>
              );
            })}
      </div>

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <Card
          title="Diagnostics"
          icon={Stethoscope}
          actions={
            <button className="btn-primary px-3 py-1.5 text-xs" disabled={diagnosing} onClick={diagnose}>
              {diagnosing && <Loader2 size={13} className="animate-spin" />} Run diagnostics
            </button>
          }
        >
          {!checks ? (
            <p className="text-[13px] text-muted">Checks the connection step by step: adapter, IP address, router, internet and DNS, and says where it breaks.</p>
          ) : (
            <ul className="space-y-2">
              {checks.map((c) => (
                <li key={c.id} className="flex gap-3 rounded-xl bg-raised/60 px-3 py-2.5">
                  {c.ok ? <CheckCircle2 size={16} className="mt-0.5 shrink-0 text-good" /> : <XCircle size={16} className="mt-0.5 shrink-0 text-bad" />}
                  <div className="min-w-0 flex-1 text-[13px]">
                    <div className="font-medium">{c.label}</div>
                    <div className="text-xs text-muted">{c.detail}</div>
                    {!c.ok && c.advice && <div className="mt-1 text-xs text-warn">{c.advice}</div>}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card
          title="DNS speed test"
          icon={Gauge}
          actions={
            <button className="btn-primary px-3 py-1.5 text-xs" disabled={benching} onClick={benchmark}>
              {benching && <Loader2 size={13} className="animate-spin" />} {bench ? "Test again" : "Run test"}
            </button>
          }
        >
          {!bench ? (
            <p className="text-[13px] text-muted">
              Times real lookups of popular sites (Google, Steam, Riot, Discord…) against your current DNS and well-known public resolvers. Faster DNS makes pages and
              game launchers start loading sooner; it does not change in-game ping.
            </p>
          ) : (
            <table className="w-full text-[13px]">
              <thead>
                <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
                  <th className="pb-1.5 font-semibold">Resolver</th>
                  <th className="pb-1.5 text-right font-semibold">Typical</th>
                  <th className="pb-1.5 text-right font-semibold">First lookup</th>
                  <th className="pb-1.5 text-right font-semibold">Failed</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {bench.map((b, i) => (
                  <tr key={b.server} className="border-t border-line/40">
                    <td className="py-2">
                      {b.name} {i === 0 && <Badge tone="good">Fastest</Badge>}
                      <div className="font-mono text-[11px] text-muted">{b.server}</div>
                    </td>
                    <td className="py-2 text-right tabular-nums">{b.cachedMs != null ? `${b.cachedMs.toFixed(0)} ms` : "—"}</td>
                    <td className="py-2 text-right tabular-nums text-muted">{b.uncachedMs != null ? `${b.uncachedMs.toFixed(0)} ms` : "—"}</td>
                    <td className={`py-2 text-right tabular-nums ${b.failures ? "text-warn" : "text-muted"}`}>
                      {b.failures}/{b.queries}
                    </td>
                    <td className="py-2 pl-2 text-right">
                      {!b.current && primary && (
                        <button className="text-xs text-accent hover:underline" onClick={() => setDnsFor({ a: primary, initial: [b.server] })}>
                          Use
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
      </div>

      <Card title="Repair tools" icon={Wrench}>
        <p className="mb-3 text-[13px] text-muted">Start at the top: each step is more drastic than the one before. Every tool explains its effects before it runs.</p>
        <div className="flex flex-wrap gap-2">
          <OperationButton op="maint.flush_dns">Flush DNS cache</OperationButton>
          <OperationButton op="net.renew_ip" onDone={() => load()}>
            Release and renew IP
          </OperationButton>
          <OperationButton op="net.reset_winsock">Reset Winsock</OperationButton>
          <OperationButton op="net.reset_tcpip">Reset TCP/IP</OperationButton>
          <OperationButton op="diag.ipconfig">Show full configuration</OperationButton>
        </div>
      </Card>

      {dnsFor && presets.length > 0 && <DnsDialog adapter={dnsFor.a} presets={presets} initial={dnsFor.initial} onClose={() => setDnsFor(null)} onDone={load} />}

      <Modal
        open={!!confirm}
        title={
          confirm?.action === "disable"
            ? `Disable ${confirm.a.name}?`
            : confirm?.action === "enable"
              ? `Enable ${confirm?.a.name}?`
              : `Make ${confirm?.a.networkName ?? confirm?.a.name} a ${confirm?.action} network?`
        }
        onClose={() => setConfirm(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirm(null)}>
              Cancel
            </button>
            <button className="btn-primary" onClick={doConfirm}>
              Confirm
            </button>
          </>
        }
      >
        {confirm && (
          <p className="text-[13px] leading-relaxed">
            {confirm.action === "disable" &&
              `The adapter stops working until you enable it again here.${confirm.a === primary ? " This is the connection you are using now: you will be offline." : ""}`}
            {confirm.action === "enable" && "The adapter is turned back on and reconnects automatically."}
            {confirm.action === "private" && "Private lets other devices on this network find this PC (file sharing, casting, LAN games). Use it only on networks you trust, like your home."}
            {confirm.action === "public" && "Public hides this PC from other devices on the network. Recommended for cafés, hotels and other shared networks."}
          </p>
        )}
      </Modal>
    </div>
  );
}
