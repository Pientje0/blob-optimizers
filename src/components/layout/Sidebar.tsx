import { Loader2, ShieldAlert, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { NAV, type PageId } from "../../navigation";
import { useEngine } from "../../context/EngineContext";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { api, errorMessage } from "../../lib/api";
import { BlobLogo } from "../ui/BlobLogo";
import { Modal } from "../ui/Modal";

interface Props {
  current: PageId;
  onNavigate: (id: PageId) => void;
}

export function Sidebar({ current, onNavigate }: Props) {
  const { appInfo } = useSystem();
  const { running } = useEngine();
  const toast = useToast();
  const [askRelaunch, setAskRelaunch] = useState(false);

  const relaunch = async () => {
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      toast("warn", "Could not relaunch as administrator", errorMessage(e));
    } finally {
      setAskRelaunch(false);
    }
  };

  return (
    <aside className="flex w-[232px] shrink-0 flex-col border-r border-line/60 bg-surface/60">
      <div className="flex items-center gap-3 px-5 pb-4 pt-5">
        <BlobLogo className="h-9 w-9" />
        <div className="leading-tight">
          <div className="text-[15px] font-semibold tracking-tight">Blob Optimizers</div>
          <div className="text-[11px] text-muted">v{appInfo?.version ?? "…"}</div>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-3 pb-3">
        {NAV.map((group) => (
          <div key={group.title} className="mt-3">
            <div className="label px-3 pb-1.5">{group.title}</div>
            {group.items.map((item) => {
              const active = item.id === current;
              const Icon = item.icon;
              const busy = item.id === "terminal" && running.length > 0;
              return (
                <button
                  key={item.id}
                  onClick={() => onNavigate(item.id)}
                  className={`group relative mb-0.5 flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left text-[13.5px] transition-colors duration-150 ${
                    active ? "bg-raised text-fg" : "text-muted hover:bg-raised/50 hover:text-fg"
                  }`}
                >
                  {active && (
                    <span className="absolute left-0 top-1/2 h-4 w-[3px] -translate-y-1/2 rounded-full bg-gradient-to-b from-accent to-accent2" />
                  )}
                  <Icon size={17} className={active ? "text-accent" : "opacity-80 group-hover:opacity-100"} />
                  <span className="flex-1">{item.label}</span>
                  {busy && (
                    <span className="flex items-center gap-1 rounded-full bg-accent/15 px-1.5 py-0.5 text-[10.5px] text-accent" title={`${running.length} running`}>
                      <Loader2 size={11} className="animate-spin" />
                      {running.length}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        ))}
      </nav>

      <div className="border-t border-line/60 p-3">
        {appInfo &&
          (appInfo.elevated ? (
            <div
              className="flex items-center gap-2.5 rounded-xl bg-good/10 px-3 py-2.5 text-xs text-good"
              title="Running as administrator. System changes can be applied without extra prompts."
            >
              <ShieldCheck size={15} /> Administrator
            </div>
          ) : (
            <button
              onClick={() => setAskRelaunch(true)}
              className="flex w-full items-center gap-2.5 rounded-xl bg-raised px-3 py-2.5 text-left text-xs text-muted transition-colors hover:text-fg"
              title="Operations that need administrator rights will show a Windows permission prompt. Click to restart as administrator instead."
            >
              <ShieldAlert size={15} />
              <span className="flex-1">Standard user</span>
              <span className="text-[10.5px] text-accent">Elevate</span>
            </button>
          ))}
      </div>

      <Modal
        open={askRelaunch}
        title="Restart as administrator?"
        onClose={() => setAskRelaunch(false)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setAskRelaunch(false)}>
              Not now
            </button>
            <button className="btn-primary" onClick={relaunch}>
              <ShieldCheck size={15} /> Restart elevated
            </button>
          </>
        }
      >
        Blob Optimizers will close and reopen with administrator rights after you approve the Windows prompt. Operations that need
        administrator rights then run without a separate prompt each time. Without this, you are asked only when such an operation
        starts.
      </Modal>
    </aside>
  );
}
