import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { createContext, useCallback, useContext, useState, type ReactNode } from "react";

type Kind = "success" | "error" | "info" | "warn";

interface Toast {
  id: number;
  kind: Kind;
  title: string;
  detail?: string;
}

const ToastContext = createContext<((kind: Kind, title: string, detail?: string) => void) | null>(null);

const ICON = {
  success: <CheckCircle2 size={17} className="text-good" />,
  error: <XCircle size={17} className="text-bad" />,
  warn: <AlertTriangle size={17} className="text-warn" />,
  info: <Info size={17} className="text-accent" />,
};

let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  const show = useCallback(
    (kind: Kind, title: string, detail?: string) => {
      const id = nextId++;
      setToasts((t) => [...t.slice(-3), { id, kind, title, detail }]);
      window.setTimeout(() => dismiss(id), kind === "error" ? 9000 : 5000);
    },
    [dismiss],
  );

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div className="pointer-events-none fixed bottom-5 right-5 z-50 flex w-[380px] flex-col gap-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            className="card animate-fade-up pointer-events-auto flex items-start gap-3 bg-raised px-4 py-3 shadow-2xl shadow-black/50"
          >
            <span className="mt-0.5">{ICON[t.kind]}</span>
            <div className="min-w-0 flex-1">
              <div className="text-[13.5px] font-medium">{t.title}</div>
              {t.detail && <div className="selectable mt-0.5 text-xs leading-relaxed text-muted">{t.detail}</div>}
            </div>
            <button className="text-muted hover:text-fg" onClick={() => dismiss(t.id)} aria-label="Dismiss">
              <X size={15} />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used inside ToastProvider");
  return ctx;
}
