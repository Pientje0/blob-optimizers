import { CornerDownLeft, OctagonX, ShieldCheck, TriangleAlert } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { api, errorMessage, type Shell, type Validation } from "../../lib/api";
import { Modal } from "../ui/Modal";

const RECALL_KEY = "blob.terminal.recall";
const RECALL_MAX = 100;

function loadRecall(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECALL_KEY) ?? "[]");
  } catch {
    return [];
  }
}

export function CommandInput({ onStarted }: { onStarted: (id: string) => void }) {
  const { runCommand } = useEngine();
  const { appInfo } = useSystem();
  const toast = useToast();
  const [shell, setShell] = useState<Shell>("powershell");
  const [command, setCommand] = useState("");
  const [asAdmin, setAsAdmin] = useState(false);
  const [validation, setValidation] = useState<Validation | null>(null);
  const [confirm, setConfirm] = useState<Validation | null>(null);
  const [busy, setBusy] = useState(false);
  const recall = useRef<string[]>(loadRecall());
  const recallIndex = useRef(-1);

  // Live safety feedback while typing (the backend re-checks on submit).
  useEffect(() => {
    if (!command.trim()) {
      setValidation(null);
      return;
    }
    const t = window.setTimeout(() => {
      api.validateCommand(shell, command).then(setValidation).catch(() => setValidation(null));
    }, 200);
    return () => window.clearTimeout(t);
  }, [shell, command]);

  const remember = (cmd: string) => {
    recall.current = [cmd, ...recall.current.filter((c) => c !== cmd)].slice(0, RECALL_MAX);
    recallIndex.current = -1;
    try {
      localStorage.setItem(RECALL_KEY, JSON.stringify(recall.current));
    } catch {
      /* not critical */
    }
  };

  const submit = async (confirmed: boolean) => {
    const cmd = command.trim();
    if (!cmd || busy) return;
    const v = await api.validateCommand(shell, cmd).catch(() => null);
    if (v?.level === "block") {
      toast("error", "Blocked for safety", v.reasons.join(" "));
      return;
    }
    if (v?.level === "warn" && !confirmed) {
      setConfirm(v);
      return;
    }
    setBusy(true);
    try {
      const entry = await runCommand(shell, cmd, confirmed, asAdmin && !appInfo?.elevated);
      remember(cmd);
      setCommand("");
      setConfirm(null);
      onStarted(entry.id);
    } catch (e) {
      toast("error", "Command not started", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      submit(false);
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const list = recall.current;
      if (!list.length) return;
      e.preventDefault();
      const next =
        e.key === "ArrowUp" ? Math.min(recallIndex.current + 1, list.length - 1) : Math.max(recallIndex.current - 1, -1);
      recallIndex.current = next;
      setCommand(next === -1 ? "" : list[next]);
    }
  };

  return (
    <div className="border-t border-line/60 bg-surface px-4 py-3">
      {validation && validation.level !== "ok" && (
        <div className={`mb-2 flex items-start gap-2 text-xs ${validation.level === "block" ? "text-bad" : "text-warn"}`}>
          {validation.level === "block" ? <OctagonX size={14} className="mt-px shrink-0" /> : <TriangleAlert size={14} className="mt-px shrink-0" />}
          <span>
            {validation.level === "block" ? "Blocked: " : "Needs confirmation: "}
            {validation.reasons.join(" ")}
          </span>
        </div>
      )}
      <div className="flex items-center gap-2">
        <div className="flex rounded-xl border border-line bg-base/60 p-0.5 text-xs">
          {(["powershell", "cmd"] as Shell[]).map((s) => (
            <button
              key={s}
              onClick={() => setShell(s)}
              className={`rounded-[10px] px-2.5 py-1.5 font-medium transition-colors ${shell === s ? "bg-raised text-fg" : "text-muted hover:text-fg"}`}
            >
              {s === "powershell" ? "PowerShell" : "CMD"}
            </button>
          ))}
        </div>
        <div className="flex flex-1 items-center rounded-xl border border-line bg-base/60 focus-within:border-accent/60">
          <span className="pl-3 font-mono text-[12.5px] text-accent">{shell === "cmd" ? ">" : "PS>"}</span>
          <input
            value={command}
            onChange={(e) => {
              setCommand(e.target.value);
              recallIndex.current = -1;
            }}
            onKeyDown={onKeyDown}
            spellCheck={false}
            placeholder={shell === "cmd" ? "Type a CMD command and press Enter" : "Type a PowerShell command and press Enter"}
            className="min-w-0 flex-1 bg-transparent px-2 py-2 font-mono text-[12.5px] outline-none placeholder:font-sans placeholder:text-muted"
          />
        </div>
        {!appInfo?.elevated && (
          <label className="flex cursor-pointer items-center gap-1.5 text-xs text-muted" title="Shows a Windows permission prompt">
            <input type="checkbox" checked={asAdmin} onChange={(e) => setAsAdmin(e.target.checked)} className="accent-[rgb(var(--c-accent))]" />
            <ShieldCheck size={13} /> Admin
          </label>
        )}
        <button className="btn-primary px-3" disabled={!command.trim() || busy || validation?.level === "block"} onClick={() => submit(false)}>
          <CornerDownLeft size={15} />
        </button>
      </div>

      <Modal
        open={!!confirm}
        title="Run this command?"
        onClose={() => setConfirm(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setConfirm(null)}>
              Cancel
            </button>
            <button className="btn-primary bg-warn text-black" disabled={busy} onClick={() => submit(true)}>
              I understand, run it
            </button>
          </>
        }
      >
        <div className="space-y-4">
          <pre className="whitespace-pre-wrap break-all rounded-xl border border-line bg-base/70 px-3 py-2.5 font-mono text-[12px]">{command}</pre>
          <ul className="space-y-2">
            {confirm?.reasons.map((r) => (
              <li key={r} className="flex gap-2 rounded-xl border border-warn/30 bg-warn/10 p-3 text-[13px]">
                <TriangleAlert size={16} className="mt-0.5 shrink-0 text-warn" /> {r}
              </li>
            ))}
          </ul>
          <p className="text-xs text-muted">
            Typed commands are not reviewed like built-in operations and are not automatically backed up. Prefer the matching section
            of Blob Optimizers when one exists.
          </p>
        </div>
      </Modal>
    </div>
  );
}
