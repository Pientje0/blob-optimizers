import { useEffect, useRef, useState } from "react";
import { ArrowDown } from "lucide-react";
import type { OutputLine } from "../../lib/api";

const STREAM_CLASS = {
  out: "text-fg/90",
  err: "text-bad",
  info: "text-accent",
};

/** Console-style output that follows new lines unless the user has scrolled up. */
export function OutputView({ lines, empty }: { lines: OutputLine[]; empty?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [follow, setFollow] = useState(true);

  useEffect(() => {
    if (follow && ref.current) ref.current.scrollTop = ref.current.scrollHeight;
  }, [lines, follow]);

  const onScroll = () => {
    const el = ref.current;
    if (!el) return;
    setFollow(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
  };

  return (
    <div className="relative min-h-0 flex-1">
      <div
        ref={ref}
        onScroll={onScroll}
        className="selectable absolute inset-0 overflow-auto bg-base/80 px-4 py-3 font-mono text-[12.5px] leading-[1.55]"
      >
        {lines.length === 0 ? (
          <div className="text-muted">{empty ?? "No output yet."}</div>
        ) : (
          lines.map((l, i) => (
            <div key={i} className={`whitespace-pre-wrap break-words ${STREAM_CLASS[l.stream]}`}>
              {l.text || " "}
            </div>
          ))
        )}
      </div>
      {!follow && (
        <button
          className="btn-ghost absolute bottom-3 right-4 px-2.5 py-1 text-xs shadow-lg"
          onClick={() => setFollow(true)}
        >
          <ArrowDown size={13} /> Follow output
        </button>
      )}
    </div>
  );
}
