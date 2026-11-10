import { Box, Plus, Settings, Shirt } from "lucide-react";
import type { InstanceConfig, ModLoader } from "../lib/types";
import { cx } from "./ui";

const LOADER_COLORS: Record<ModLoader, string> = {
  vanilla: "from-emerald-500 to-green-700",
  fabric: "from-amber-200 to-orange-400",
  forge: "from-slate-400 to-slate-700",
  neoforge: "from-orange-500 to-red-700",
};

export function LoaderBadge({ loader }: { loader: ModLoader }) {
  const label = { vanilla: "Vanilla", fabric: "Fabric", forge: "Forge", neoforge: "NeoForge" }[loader];
  return (
    <span className="rounded bg-panel-3 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-muted">
      {label}
    </span>
  );
}

interface Props {
  instances: InstanceConfig[];
  selected: string | null;
  running: Set<string>;
  preparing: Set<string>;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onSettings: () => void;
  onWardrobe: () => void;
  wardrobeActive: boolean;
}

export function InstanceSidebar({ instances, selected, running, preparing, onSelect, onCreate, onSettings, onWardrobe, wardrobeActive }: Props) {
  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-line bg-panel">
      <div className="flex items-center gap-2.5 px-4 py-4">
        <div className="grid h-8 w-8 place-items-center rounded-lg bg-gradient-to-br from-accent to-fuchsia-500 shadow-lg shadow-accent/30">
          <Box size={18} />
        </div>
        <span className="text-[15px] font-bold tracking-tight">Aurora</span>
      </div>

      <div className="flex items-center justify-between px-4 pb-2 pt-1">
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted">Instâncias</span>
        <button
          onClick={onCreate}
          title="Nova instância"
          className="rounded p-1 text-muted transition hover:bg-panel-3 hover:text-white"
        >
          <Plus size={16} />
        </button>
      </div>

      <nav className="flex-1 space-y-0.5 overflow-y-auto px-2 pb-2">
        {instances.length === 0 && (
          <button
            onClick={onCreate}
            className="mt-2 w-full rounded-lg border border-dashed border-line px-3 py-6 text-sm text-muted transition hover:border-accent hover:text-white"
          >
            Crie sua primeira instância
          </button>
        )}
        {instances.map((inst) => {
          const isRunning = running.has(inst.id);
          const isPreparing = preparing.has(inst.id);
          return (
            <button
              key={inst.id}
              onClick={() => onSelect(inst.id)}
              className={cx(
                "group flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition",
                selected === inst.id && !wardrobeActive ? "bg-panel-3 text-white" : "text-zinc-400 hover:bg-panel-2 hover:text-zinc-100",
              )}
            >
              <span
                className={cx(
                  "relative grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-gradient-to-br text-xs font-bold text-black/70",
                  LOADER_COLORS[inst.loader],
                )}
              >
                {inst.name.slice(0, 2).toUpperCase()}
                {(isRunning || isPreparing) && (
                  <span
                    className={cx(
                      "absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full border-2 border-panel",
                      isRunning ? "bg-ok" : "animate-pulse bg-warn",
                    )}
                  />
                )}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium">{inst.name}</span>
                <span className="flex items-center gap-1.5 text-xs text-muted">
                  {inst.minecraftVersion} <LoaderBadge loader={inst.loader} />
                </span>
              </span>
            </button>
          );
        })}
      </nav>

      <button
        onClick={onWardrobe}
        className={cx(
          "flex items-center gap-2 border-t border-line px-4 py-3 text-sm transition hover:bg-panel-2 hover:text-white",
          wardrobeActive ? "bg-panel-2 text-white" : "text-muted",
        )}
      >
        <Shirt size={16} /> Guarda-roupa
      </button>
      <button
        onClick={onSettings}
        className="flex items-center gap-2 border-t border-line px-4 py-3 text-sm text-muted transition hover:bg-panel-2 hover:text-white"
      >
        <Settings size={16} /> Configurações
      </button>
    </aside>
  );
}
