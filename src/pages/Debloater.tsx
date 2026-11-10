import { CheckCheck, RefreshCw, XCircle } from "lucide-react";
import { useMemo, useState } from "react";
import { AppTable } from "../components/apps/AppTable";
import { FeaturesPanel } from "../components/apps/FeaturesPanel";
import { RemovalDialog } from "../components/apps/RemovalDialog";
import { RemovedAppsPanel } from "../components/apps/RemovedAppsPanel";
import { SelectionBar } from "../components/apps/SelectionBar";
import { Skeleton } from "../components/ui/Card";
import { Chip, SearchInput, Switch, Tabs } from "../components/ui/Controls";
import { useApps } from "../hooks/useApps";
import type { InstalledApp } from "../lib/api";
import type { PageId } from "../navigation";

type Tab = "apps" | "features" | "removed";
type Filter = "preinstalled" | "microsoft" | "gaming" | "thirdParty" | "all";

const FILTERS: { id: Filter; label: string; test: (a: InstalledApp) => boolean }[] = [
  { id: "preinstalled", label: "Preinstalled", test: (a) => a.preinstalled },
  { id: "microsoft", label: "Microsoft", test: (a) => a.category === "microsoft" },
  { id: "gaming", label: "Gaming", test: (a) => a.category === "gaming" },
  { id: "thirdParty", label: "Third-party", test: (a) => a.category === "thirdParty" },
  { id: "all", label: "All installed", test: () => true },
];

const REC_ORDER = { safe: 0, optional: 1, caution: 2, protected: 3 };

export function Debloater({ onNavigate }: { onNavigate: (id: PageId) => void }) {
  const { apps, loading, error, refresh } = useApps();
  const [tab, setTab] = useState<Tab>("apps");
  const [filter, setFilter] = useState<Filter>("preinstalled");
  const [query, setQuery] = useState("");
  const [showProtected, setShowProtected] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState<InstalledApp[] | null>(null);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    const f = FILTERS.find((x) => x.id === filter)!;
    return (apps ?? [])
      .filter((a) => (showProtected || a.recommendation !== "protected") && f.test(a))
      .filter((a) => !q || a.name.toLowerCase().includes(q) || a.publisher.toLowerCase().includes(q) || (a.packageName ?? "").toLowerCase().includes(q))
      .sort((a, b) => REC_ORDER[a.recommendation] - REC_ORDER[b.recommendation] || a.name.localeCompare(b.name));
  }, [apps, filter, query, showProtected]);

  const counts = useMemo(
    () => Object.fromEntries(FILTERS.map((f) => [f.id, (apps ?? []).filter((a) => a.recommendation !== "protected" && f.test(a)).length])),
    [apps],
  );
  const safeVisible = visible.filter((a) => a.recommendation === "safe");

  const setOne = (id: string, on: boolean) =>
    setSelected((s) => {
      const n = new Set(s);
      if (on) n.add(id);
      else n.delete(id);
      return n;
    });
  const setMany = (ids: string[], on: boolean) =>
    setSelected((s) => {
      const n = new Set(s);
      ids.forEach((id) => (on ? n.add(id) : n.delete(id)));
      return n;
    });

  const startRemoval = () => setRemoving((apps ?? []).filter((a) => selected.has(a.id)));

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Tabs<Tab>
          value={tab}
          onChange={setTab}
          tabs={[
            { id: "apps", label: "Apps", count: apps?.filter((a) => a.recommendation !== "protected").length },
            { id: "features", label: "Windows features" },
            { id: "removed", label: "Removed apps" },
          ]}
        />
        {tab === "apps" && (
          <button className="btn-ghost" onClick={refresh} disabled={loading}>
            <RefreshCw size={15} className={loading ? "animate-spin" : ""} /> Rescan
          </button>
        )}
      </div>

      {tab === "features" && <FeaturesPanel />}
      {tab === "removed" && <RemovedAppsPanel onChanged={refresh} />}

      {tab === "apps" && (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <SearchInput value={query} onChange={setQuery} placeholder="Search apps or publishers" className="w-64" />
            {FILTERS.map((f) => (
              <Chip key={f.id} active={filter === f.id} onClick={() => setFilter(f.id)} count={apps ? counts[f.id] : undefined}>
                {f.label}
              </Chip>
            ))}
            <span className="flex-1" />
            <label className="flex items-center gap-2 text-xs text-muted">
              <Switch label="Show protected" checked={showProtected} onChange={setShowProtected} /> Show protected
            </label>
            <button className="btn-ghost px-3 py-1.5 text-xs" disabled={!safeVisible.length} onClick={() => setMany(safeVisible.map((a) => a.id), true)}>
              <CheckCheck size={13} /> Select safe to remove ({safeVisible.length})
            </button>
          </div>

          {error && (
            <div className="flex items-center gap-2 rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">
              <XCircle size={15} /> Could not list apps: {error}
            </div>
          )}

          <div className="card overflow-hidden">
            {!apps ? (
              <div className="space-y-4 p-5">
                {Array.from({ length: 6 }).map((_, i) => (
                  <Skeleton key={i} className="h-10 w-full" />
                ))}
              </div>
            ) : (
              <AppTable
                apps={visible}
                selected={selected}
                onSelect={setOne}
                onSelectAll={(on) => setMany(visible.filter((a) => a.recommendation !== "protected").map((a) => a.id), on)}
                empty={query ? "No apps match your search." : "Nothing in this category. Your PC is already clean here."}
              />
            )}
          </div>
          <p className="text-xs text-muted">
            Protected items (Microsoft Store, Windows Security, Edge, runtimes and core Windows components) are never offered for removal. Background
            and startup apps are managed in the Startup and Processes sections.
          </p>

          <SelectionBar count={selected.size} onClear={() => setSelected(new Set())} onRemove={startRemoval} />
        </>
      )}

      {removing && (
        <RemovalDialog
          apps={removing}
          onClose={() => setRemoving(null)}
          onChanged={() => {
            setSelected(new Set());
            refresh();
          }}
          onOpenTerminal={() => onNavigate("terminal")}
        />
      )}
    </div>
  );
}
