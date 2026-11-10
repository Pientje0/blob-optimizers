import { useEffect, useRef, useState } from "react";
import { Sidebar } from "./components/layout/Sidebar";
import { EngineProvider } from "./context/EngineContext";
import { SettingsProvider } from "./context/SettingsContext";
import { SystemProvider } from "./context/SystemContext";
import { ToastProvider } from "./context/ToastContext";
import { findNav, type PageId } from "./navigation";
import { UpdateNotifier } from "./components/settings/UpdateNotifier";
import { Dashboard } from "./pages/Dashboard";
import { Hardware } from "./pages/Hardware";
import { Terminal } from "./pages/Terminal";
import { Debloater } from "./pages/Debloater";
import { Uninstaller } from "./pages/Uninstaller";
import { Tweaks } from "./pages/Tweaks";
import { Gaming } from "./pages/Gaming";
import { Network } from "./pages/Network";
import { Restore } from "./pages/Restore";
import { SettingsPage } from "./pages/SettingsPage";
import { Startup } from "./pages/Startup";
import { Processes } from "./pages/Processes";

function Page({ id, onNavigate }: { id: PageId; onNavigate: (id: PageId) => void }) {
  switch (id) {
    case "dashboard":
      return <Dashboard onNavigate={onNavigate} />;
    case "hardware":
      return <Hardware />;
    case "terminal":
      return <Terminal />;
    case "debloat":
      return <Debloater onNavigate={onNavigate} />;
    case "uninstaller":
      return <Uninstaller onNavigate={onNavigate} />;
    case "tweaks":
      return <Tweaks />;
    case "gaming":
      return <Gaming onNavigate={onNavigate} />;
    case "network":
      return <Network />;
    case "restore":
      return <Restore onNavigate={onNavigate} />;
    case "settings":
      return <SettingsPage />;
    case "startup":
      return <Startup />;
    case "processes":
      return <Processes />;
  }
}

export default function App() {
  const [page, setPage] = useState<PageId>("dashboard");
  const nav = findNav(page);
  const scroller = useRef<HTMLDivElement>(null);

  // Each page opens at the top instead of inheriting the previous page's scroll position.
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [page]);

  return (
    <ToastProvider>
      <SettingsProvider>
        <SystemProvider>
          <EngineProvider>
            <UpdateNotifier />
            <div className="flex h-full">
              <Sidebar current={page} onNavigate={setPage} />
              <main className="flex min-w-0 flex-1 flex-col">
                <header className="flex items-end justify-between border-b border-line/60 px-8 pb-4 pt-6">
                  <div>
                    <h1 className="text-[22px] font-semibold tracking-tight">{nav.label}</h1>
                    <p className="mt-0.5 text-sm text-muted">{nav.description}</p>
                  </div>
                </header>
                <div ref={scroller} className="flex-1 overflow-y-auto px-8 py-6">
                  {/* key forces the entry animation to replay on navigation */}
                  <div key={page} className="mx-auto max-w-[1600px]">
                    <Page id={page} onNavigate={setPage} />
                  </div>
                </div>
              </main>
            </div>
          </EngineProvider>
        </SystemProvider>
      </SettingsProvider>
    </ToastProvider>
  );
}
