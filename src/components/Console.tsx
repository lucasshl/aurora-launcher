import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ArrowDownToLine, Copy, Search, Trash2 } from "lucide-react";
import type { LogLevel, LogLine } from "../lib/types";
import { cx } from "./ui";

const LEVEL_STYLE: Record<LogLevel, string> = {
  info: "text-zinc-300",
  debug: "text-zinc-500",
  warn: "text-warn",
  error: "text-err",
};

const RENDER_LIMIT = 1500;

export function Console({ lines, onClear }: { lines: LogLine[]; onClear: () => void }) {
  const [query, setQuery] = useState("");
  const [levels, setLevels] = useState<Record<LogLevel, boolean>>({ info: true, warn: true, error: true, debug: false });
  const [follow, setFollow] = useState(true);
  const scroller = useRef<HTMLDivElement>(null);

  const visible = useMemo(() => {
    const q = query.toLowerCase();
    const filtered = lines.filter((l) => levels[l.level] && (!q || l.line.toLowerCase().includes(q)));
    return filtered.length > RENDER_LIMIT ? filtered.slice(-RENDER_LIMIT) : filtered;
  }, [lines, query, levels]);

  useEffect(() => {
    if (follow && scroller.current) scroller.current.scrollTop = scroller.current.scrollHeight;
  }, [visible, follow]);

  const errors = useMemo(() => lines.filter((l) => l.level === "error").length, [lines]);

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden rounded-xl border border-line bg-[#0a0b0e]">
      <div className="flex items-center gap-2 border-b border-line px-3 py-2">
        <span className="text-xs font-semibold uppercase tracking-wider text-muted">Console</span>
        {errors > 0 && <span className="rounded bg-err/15 px-1.5 py-0.5 text-[10px] font-bold text-err">{errors} erros</span>}
        <div className="ml-2 flex gap-1">
          {(Object.keys(levels) as LogLevel[]).map((lvl) => (
            <button
              key={lvl}
              onClick={() => setLevels((s) => ({ ...s, [lvl]: !s[lvl] }))}
              className={cx(
                "rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase transition",
                levels[lvl] ? cx("bg-panel-3", LEVEL_STYLE[lvl]) : "text-zinc-600 hover:text-zinc-400",
              )}
            >
              {lvl}
            </button>
          ))}
        </div>
        <div className="ml-auto flex items-center gap-1">
          <div className="relative">
            <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-muted" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Filtrar"
              className="w-40 rounded bg-panel-2 py-1 pl-7 pr-2 text-xs outline-none focus:ring-1 focus:ring-accent"
            />
          </div>
          <IconBtn title="Seguir saída" active={follow} onClick={() => setFollow((f) => !f)}>
            <ArrowDownToLine size={14} />
          </IconBtn>
          <IconBtn title="Copiar tudo" onClick={() => void navigator.clipboard.writeText(lines.map((l) => l.line).join("\n"))}>
            <Copy size={14} />
          </IconBtn>
          <IconBtn title="Limpar" onClick={onClear}>
            <Trash2 size={14} />
          </IconBtn>
        </div>
      </div>
      <div
        ref={scroller}
        onWheel={(e) => e.deltaY < 0 && setFollow(false)}
        className="selectable min-h-0 flex-1 overflow-y-auto px-3 py-2 font-mono text-[11.5px] leading-[1.55]"
      >
        {visible.length === 0 ? (
          <p className="pt-6 text-center text-muted">Os logs do jogo aparecem aqui.</p>
        ) : (
          visible.map((l, i) => (
            <div
              key={i}
              className={cx(
                "whitespace-pre-wrap break-all",
                LEVEL_STYLE[l.level],
                l.stream === "launcher" && "text-accent/90",
              )}
            >
              {l.stream === "launcher" && <span className="mr-1 text-accent">›</span>}
              {l.line}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function IconBtn({ children, title, onClick, active }: { children: ReactNode; title: string; onClick: () => void; active?: boolean }) {
  return (
    <button
      title={title}
      onClick={onClick}
      className={cx("rounded p-1.5 transition hover:bg-panel-3", active ? "text-accent" : "text-muted hover:text-white")}
    >
      {children}
    </button>
  );
}
