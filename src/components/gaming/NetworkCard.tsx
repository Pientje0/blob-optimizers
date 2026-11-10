import { AlertTriangle, CheckCircle2, Loader2, Wifi } from "lucide-react";
import { useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { api, errorMessage } from "../../lib/api";
import { Card } from "../ui/Card";

interface Result {
  host: string;
  sent: number;
  lost: number;
  min: number;
  avg: number;
  max: number;
  jitter: number;
}

const HOSTS = [
  { host: "1.1.1.1", label: "Cloudflare" },
  { host: "8.8.8.8", label: "Google" },
];

/**
 * Parses `ping` output. Works with localized output: reply lines contain exactly one "=12ms"/"<1ms"
 * value, while the summary line contains three (min/max/avg) and is skipped.
 */
function parse(host: string, lines: string[], sent: number): Result {
  const t = lines
    .filter((l) => (l.match(/\d+\s?ms/gi) ?? []).length === 1)
    .map((l) => l.match(/[=<](\d+)\s?ms/i))
    .filter((m): m is RegExpMatchArray => !!m)
    .map((m) => +m[1]);
  if (!t.length) return { host, sent, lost: sent, min: 0, avg: 0, max: 0, jitter: 0 };
  const avg = t.reduce((a, b) => a + b, 0) / t.length;
  const jitter = t.length > 1 ? t.slice(1).reduce((s, v, i) => s + Math.abs(v - t[i]), 0) / (t.length - 1) : 0;
  return { host, sent, lost: Math.max(0, sent - t.length), min: Math.min(...t), avg, max: Math.max(...t), jitter };
}

export function NetworkCard() {
  const { hardware: hw } = useSystem();
  const { runOperation, waitFor } = useEngine();
  const toast = useToast();
  const [results, setResults] = useState<Result[]>([]);
  const [testing, setTesting] = useState(false);

  const up = hw?.network.filter((n) => n.status === "Up") ?? [];
  const wired = up.some((n) => n.mediaType === "802.3");
  const wifiOnly = up.length > 0 && !wired;
  const hasEthernet = hw?.network.some((n) => n.mediaType === "802.3");

  const test = async () => {
    setTesting(true);
    setResults([]);
    try {
      const out: Result[] = [];
      for (const h of HOSTS) {
        const e = await runOperation("diag.ping", { host: h.host, count: "10" });
        await waitFor(e.id);
        const lines = (await api.jobOutput(e.id)).map((l) => l.text);
        out.push(parse(h.host, lines, 10));
        setResults([...out]);
      }
    } catch (e) {
      toast("error", "Latency test failed", errorMessage(e));
    } finally {
      setTesting(false);
    }
  };

  return (
    <Card
      title="Network for gaming"
      icon={Wifi}
      className="h-full"
      actions={
        <button className="btn-ghost px-3 py-1.5 text-xs" disabled={testing} onClick={test}>
          {testing && <Loader2 size={13} className="animate-spin" />} Test latency
        </button>
      }
    >
      <ul className="space-y-2 text-xs leading-relaxed">
        {wifiOnly ? (
          <li className="flex gap-2">
            <AlertTriangle size={14} className="mt-px shrink-0 text-warn" />
            <span>
              You are on Wi-Fi{hasEthernet ? " although this PC has an Ethernet port" : ""}. A cable gives lower and, above all, more stable latency (less jitter
              and packet loss) in online games.
            </span>
          </li>
        ) : wired ? (
          <li className="flex gap-2 text-muted">
            <CheckCircle2 size={14} className="mt-px shrink-0 text-good" /> Connected by cable.
          </li>
        ) : null}
        <li className="flex gap-2 text-muted">
          <CheckCircle2 size={14} className="mt-px shrink-0 text-good" />
          Downloads, cloud sync and streams on the same network compete with your game. Pause them while playing; Windows Update peer sharing can be turned off
          under Windows Tweaks → Windows.
        </li>
      </ul>
      {results.length > 0 && (
        <table className="mt-4 w-full text-[13px]">
          <thead>
            <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
              <th className="pb-1.5 font-semibold">Server</th>
              <th className="pb-1.5 text-right font-semibold">Avg</th>
              <th className="pb-1.5 text-right font-semibold">Jitter</th>
              <th className="pb-1.5 text-right font-semibold">Loss</th>
            </tr>
          </thead>
          <tbody>
            {results.map((r) => (
              <tr key={r.host} className="border-t border-line/40">
                <td className="py-1.5">{HOSTS.find((h) => h.host === r.host)?.label ?? r.host}</td>
                <td className={`py-1.5 text-right tabular-nums ${r.avg > 60 ? "text-warn" : ""}`}>{r.lost === r.sent ? "—" : `${r.avg.toFixed(0)} ms`}</td>
                <td className={`py-1.5 text-right tabular-nums ${r.jitter > 10 ? "text-warn" : ""}`}>{r.lost === r.sent ? "—" : `${r.jitter.toFixed(1)} ms`}</td>
                <td className={`py-1.5 text-right tabular-nums ${r.lost ? "text-bad" : ""}`}>{Math.round((r.lost / r.sent) * 100)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {results.length > 0 && (
        <p className="mt-2 text-xs text-muted">Jitter above ~10 ms or any packet loss causes rubber-banding more often than a high average. Game servers may be farther away than these.</p>
      )}
    </Card>
  );
}
