import { AlertTriangle, Loader2, ShieldCheck } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useEngine } from "../../context/EngineContext";
import { useSystem } from "../../context/SystemContext";
import { useJob } from "../../hooks/useJob";
import { Modal } from "./Modal";

interface Props {
  /** Catalog operation id. */
  op: string;
  params?: Record<string, string>;
  children: ReactNode;
  className?: string;
  /** Always confirm, even for operations that don't change the system. */
  confirm?: boolean;
  onDone?: (ok: boolean) => void;
}

/** Runs a reviewed catalog operation, showing its effects first when it changes the system. */
export function OperationButton({ op, params, children, className = "btn-ghost px-3 py-1.5 text-xs", confirm, onDone }: Props) {
  const { catalog, runOperation } = useEngine();
  const { appInfo } = useSystem();
  const { run, busy } = useJob();
  const [open, setOpen] = useState(false);
  const operation = catalog.find((o) => o.id === op);

  const start = async () => {
    setOpen(false);
    const done = await run(op, () => runOperation(op, params), {
      success: `${operation?.title ?? op} finished`,
      failure: `${operation?.title ?? op} did not complete`,
    });
    onDone?.(done?.status === "succeeded");
  };

  return (
    <>
      <button
        className={className}
        disabled={!operation || !!busy}
        onClick={() => (operation?.modifiesSystem || confirm ? setOpen(true) : start())}
        title={operation?.description}
      >
        {busy && <Loader2 size={13} className="animate-spin" />}
        {children}
      </button>
      <Modal
        open={open}
        title={`${operation?.title}?`}
        onClose={() => setOpen(false)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setOpen(false)}>
              Cancel
            </button>
            <button className={operation?.risk === "high" ? "btn-primary bg-bad" : "btn-primary"} onClick={start}>
              Run
            </button>
          </>
        }
      >
        {operation && (
          <div className="space-y-3 text-[13px] leading-relaxed">
            <p>{operation.description}</p>
            <p className={`flex gap-2 rounded-xl border p-3 ${operation.risk === "high" ? "border-bad/30 bg-bad/10" : "border-warn/30 bg-warn/10"}`}>
              <AlertTriangle size={16} className={`mt-0.5 shrink-0 ${operation.risk === "high" ? "text-bad" : "text-warn"}`} /> {operation.effects}
            </p>
            {operation.requiresAdmin && !appInfo?.elevated && (
              <p className="flex items-center gap-2 text-xs text-muted">
                <ShieldCheck size={14} className="text-accent" /> Windows will ask for administrator permission.
              </p>
            )}
          </div>
        )}
      </Modal>
    </>
  );
}
