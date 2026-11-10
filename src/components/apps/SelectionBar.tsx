import { Trash2, X } from "lucide-react";

export function SelectionBar({ count, onClear, onRemove, label = "Remove" }: { count: number; onClear: () => void; onRemove: () => void; label?: string }) {
  if (!count) return null;
  return (
    <div className="sticky bottom-0 z-20 -mx-1 mt-4 flex animate-fade-up items-center gap-3 rounded-2xl border border-accent/30 bg-raised/95 px-4 py-3 shadow-2xl shadow-black/50 backdrop-blur">
      <span className="text-[13px] font-medium">
        {count} selected
      </span>
      <button className="flex items-center gap-1 text-xs text-muted hover:text-fg" onClick={onClear}>
        <X size={13} /> Clear
      </button>
      <span className="flex-1" />
      <button className="btn-primary bg-bad" onClick={onRemove}>
        <Trash2 size={15} /> {label} {count}
      </button>
    </div>
  );
}
