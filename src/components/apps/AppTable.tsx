import { AppWindow, Package } from "lucide-react";
import type { InstalledApp } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { Checkbox } from "../ui/Controls";
import { AppTypeBadges, RecommendationBadge } from "./AppBadges";

interface Props {
  apps: InstalledApp[];
  selected: Set<string>;
  onSelect: (id: string, on: boolean) => void;
  onSelectAll?: (on: boolean) => void;
  details?: boolean;
  empty: string;
}

export function AppTable({ apps, selected, onSelect, onSelectAll, details, empty }: Props) {
  const selectable = apps.filter((a) => a.recommendation !== "protected");
  const allSelected = selectable.length > 0 && selectable.every((a) => selected.has(a.id));

  if (apps.length === 0) return <p className="py-12 text-center text-sm text-muted">{empty}</p>;

  return (
    <table className="w-full table-fixed text-[13px]">
      <thead className="sticky top-0 z-10 bg-surface">
        <tr className="text-left text-[11px] uppercase tracking-wider text-muted">
          <th className="w-11 py-2 pl-4">
            {onSelectAll && <Checkbox checked={allSelected} onChange={onSelectAll} label="Select all" disabled={!selectable.length} />}
          </th>
          <th className="py-2 pr-3 font-semibold">App</th>
          {details && <th className="w-[200px] whitespace-nowrap px-3 py-2 font-semibold">Publisher</th>}
          {details && <th className="w-[90px] whitespace-nowrap px-3 py-2 text-right font-semibold">Size</th>}
          {details && <th className="w-[110px] whitespace-nowrap px-3 py-2 font-semibold">Installed</th>}
          <th className="w-[160px] py-2 pl-3 pr-4 text-right font-semibold">Recommendation</th>
        </tr>
      </thead>
      <tbody>
        {apps.map((a) => {
          const locked = a.recommendation === "protected";
          const on = selected.has(a.id);
          return (
            <tr
              key={a.id}
              onClick={() => !locked && onSelect(a.id, !on)}
              className={`border-t border-line/40 ${locked ? "opacity-60" : "cursor-pointer hover:bg-raised/50"} ${on ? "bg-accent/[0.06]" : ""}`}
            >
              <td className="py-2.5 pl-4 align-top">
                <div className="pt-0.5">
                  <Checkbox checked={on} disabled={locked} onChange={(v) => onSelect(a.id, v)} label={`Select ${a.name}`} />
                </div>
              </td>
              <td className="py-2.5 pr-3">
                <div className="flex min-w-0 items-center gap-2">
                  {a.kind === "appx" ? <Package size={14} className="shrink-0 text-accent2" /> : <AppWindow size={14} className="shrink-0 text-accent" />}
                  <span className="truncate font-medium" title={a.name}>
                    {a.name}
                  </span>
                  {a.version && <span className="hidden truncate text-[11px] text-muted xl:inline">{a.version}</span>}
                </div>
                <div className="mt-0.5 flex flex-wrap items-center gap-1.5">
                  <AppTypeBadges app={a} />
                  {!details && a.publisher && <span className="truncate text-[11.5px] text-muted">{a.publisher}</span>}
                </div>
                <div className="mt-1 text-[11.5px] leading-snug text-muted">{a.note}</div>
              </td>
              {details && (
                <td className="truncate px-3 py-2.5 align-top text-muted" title={a.publisher}>
                  {a.publisher || "—"}
                </td>
              )}
              {details && <td className="whitespace-nowrap px-3 py-2.5 text-right align-top tabular-nums text-muted">{a.sizeBytes ? formatBytes(a.sizeBytes) : "—"}</td>}
              {details && <td className="whitespace-nowrap px-3 py-2.5 align-top tabular-nums text-muted">{a.installDate ?? "—"}</td>}
              <td className="py-2.5 pl-3 pr-4 text-right align-top">
                <RecommendationBadge rec={a.recommendation} />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
