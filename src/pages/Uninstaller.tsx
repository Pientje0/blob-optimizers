import { RefreshCw, XCircle } from "lucide-react";
import { useMemo, useState } from "react";
import { AppTable } from "../components/apps/AppTable";
import { RemovalDialog } from "../components/apps/RemovalDialog";
import { SelectionBar } from "../components/apps/SelectionBar";
import { Skeleton } from "../components/ui/Card";
import { Chip, SearchInput, Switch } from "../components/ui/Controls";
import { useApps } from "../hooks/useApps";
import type { InstalledApp } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { PageId } from "../navigation";

type KindFilter = "all" | "win32" | "appx";
type Sort = "name" | "size" | "date" | "publisher";

const SORTS: Record<Sort, (a: InstalledApp, b: InstalledApp) => number> = {
  name: (a, b) => a.name.localeCompare(b.name),
  size: (a, b) => (b.sizeBytes ?? -1) - (a.sizeBytes ?? -1),
  date: (a, b) => (b.installDate ?? "").localeCompare(a.installDate ?? ""),
  publisher: (a, b) => a.publisher.localeCompare(b.publisher) || a.name.localeCompare(b.name),
};

export function Uninstaller({ onNavigate }: { onNavigate: (id: PageId) => void }) {
  const { apps, loading, error, refresh } = useApps();
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<KindFilter>("win32");
  const [sort, setSort] = useState<Sort>("name");
  const [showSystem, setShowSystem] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState<InstalledApp[] | null>(null);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (apps ?? [])
      .filter((a) => (kind === "all" || a.kind === kind) && (showSystem || a.recommendation !== "protected"))
      .filter((a) => !q || a.name.toLowerCase().includes(q) || a.publisher.toLowerCase().includes(q))
      .sort(SORTS[sort]);
  }, [apps, query, kind, sort, showSystem]);

  const totalSize = visible.reduce((s, a) => s + (a.sizeBytes ?? 0), 0);
  const setOne = (id: string, on: boolean) =>
    setSelected((s) => {
      const n = new Set(s);
      if (on) n.add(id);
      else n.delete(id);
      return n;
    });

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2">
        <SearchInput value={query} onChange={setQuery} placeholder="Search programs or publishers" className="w-64" />
        {(
          [
            ["win32", "Desktop programs"],
            ["appx", "Store apps"],
            ["all", "All"],
          ] as [KindFilter, string][]
        ).map(([id, label]) => (
          <Chip key={id} active={kind === id} onClick={() => setKind(id)} count={apps?.filter((a) => (id === "all" || a.kind === id) && a.recommendation !== "protected").length}>
            {label}
          </Chip>
        ))}
        <span className="flex-1" />
        <label className="flex items-center gap-2 text-xs text-muted">
          <Switch label="Show system components" checked={showSystem} onChange={setShowSystem} /> System components
        </label>
        <select
          value={sort}
          onChange={(e) => setSort(e.target.value as Sort)}
          className="rounded-xl border border-line bg-base/60 px-3 py-1.5 text-xs outline-none focus:border-accent/60"
          aria-label="Sort by"
        >
          <option value="name">Sort: Name</option>
          <option value="size">Sort: Size</option>
          <option value="date">Sort: Install date</option>
          <option value="publisher">Sort: Publisher</option>
        </select>
        <button className="btn-ghost px-3 py-1.5 text-xs" onClick={refresh} disabled={loading}>
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} /> Rescan
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-2 rounded-xl border border-bad/30 bg-bad/10 px-4 py-3 text-sm text-bad">
          <XCircle size={15} /> Could not list programs: {error}
        </div>
      )}

      <div className="card overflow-hidden">
        {!apps ? (
          <div className="space-y-4 p-5">
            {Array.from({ length: 8 }).map((_, i) => (
              <Skeleton key={i} className="h-10 w-full" />
            ))}
          </div>
        ) : (
          <AppTable apps={visible} selected={selected} onSelect={setOne} details empty={query ? "No programs match your search." : "No programs found."} />
        )}
      </div>
      {apps && (
        <p className="text-xs text-muted">
          {visible.length} items · {formatBytes(totalSize)} reported by installers (many programs and all Store apps do not report a size).
          After uninstalling, leftover files are detected and can be cleaned up with your confirmation.
        </p>
      )}

      <SelectionBar
        count={selected.size}
        label="Uninstall"
        onClear={() => setSelected(new Set())}
        onRemove={() => setRemoving((apps ?? []).filter((a) => selected.has(a.id)))}
      />

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
