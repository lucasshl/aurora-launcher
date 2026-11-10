import { useEffect, useState } from "react";
import { AlertTriangle, Compass, Cpu, FileCog, FolderOpen, Image, Package, Play, Settings2, Square, Terminal, Trash2, Zap } from "lucide-react";
import { EditInstanceModal } from "./EditInstanceModal";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, errorMessage } from "../lib/api";
import type { Account, ExitInfo, InstanceConfig, LogLine, ProgressEvent } from "../lib/types";
import { BrowseTab } from "./BrowseTab";
import { ConfigsTab } from "./ConfigsTab";
import { Console } from "./Console";
import { ModsTab } from "./ModsTab";
import { OptimizeTab } from "./OptimizeTab";
import { DownloadProgress } from "./DownloadProgress";
import { LoaderBadge } from "./InstanceSidebar";
import { Button, Spinner, Toggle, cx } from "./ui";

interface Props {
  instance: InstanceConfig;
  account: Account | null;
  totalMemoryMb: number;
  isRunning: boolean;
  isPreparing: boolean;
  progress?: ProgressEvent;
  speed: number;
  logs: LogLine[];
  exit?: ExitInfo;
  onPlay: () => void;
  onKill: () => void;
  onChange: (inst: InstanceConfig) => void;
  /** Salva o perfil e propaga erros (usado pelo menu de perfil). */
  onSave: (inst: InstanceConfig) => Promise<void>;
  onConvertToFabric: () => Promise<void>;
  onDelete: () => void;
  onClearLogs: () => void;
  onDismissExit: () => void;
  notify: (kind: "error" | "success" | "info", text: string) => void;
}

function timeAgo(epoch: number | null): string {
  if (!epoch) return "nunca jogada";
  const diff = Date.now() / 1000 - epoch;
  if (diff < 60) return "jogada agora";
  if (diff < 3600) return `jogada há ${Math.floor(diff / 60)} min`;
  if (diff < 86400) return `jogada há ${Math.floor(diff / 3600)} h`;
  return `jogada há ${Math.floor(diff / 86400)} dias`;
}

type Tab = "console" | "mods" | "browse" | "configs" | "optimize";

const TABS: { id: Tab; label: string; icon: typeof Terminal }[] = [
  { id: "console", label: "Console", icon: Terminal },
  { id: "mods", label: "Mods", icon: Package },
  { id: "browse", label: "Explorar", icon: Compass },
  { id: "configs", label: "Configs", icon: FileCog },
  { id: "optimize", label: "Otimização", icon: Zap },
];

export function InstancePanel(props: Props) {
  const { instance, account, isRunning, isPreparing, progress, exit } = props;
  const [tab, setTab] = useState<Tab>("console");
  const [contentVersion, setContentVersion] = useState(0);
  const [editing, setEditing] = useState(false);
  const locked = isRunning || isPreparing;
  const bumpContent = () => setContentVersion((v) => v + 1);
  const maxSlider = Math.max(2048, Math.floor((props.totalMemoryMb * 0.85) / 512) * 512);
  const [memory, setMemory] = useState(instance.memoryMaxMb);

  useEffect(() => setMemory(instance.memoryMaxMb), [instance.id, instance.memoryMaxMb]);

  const commit = (patch: Partial<InstanceConfig>) => props.onChange({ ...instance, ...patch });
  const fill = ((memory - 1024) / (maxSlider - 1024)) * 100;
  const offlineWithMods = account?.kind === "offline" && instance.loader !== "vanilla";

  const importSkin = async () => {
    if (!account) return;
    const file = await openDialog({ multiple: false, filters: [{ name: "Skin PNG", extensions: ["png"] }] });
    if (typeof file !== "string") return;
    try {
      await api.setOfflineSkin(instance.id, account.username, file);
      props.notify("success", `Skin aplicada para ${account.username} nesta instância.`);
    } catch (e) {
      props.notify("error", errorMessage(e));
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 p-6">
      {/* Cabeçalho */}
      <section className="relative shrink-0 overflow-hidden rounded-2xl border border-line bg-gradient-to-br from-panel-2 via-panel to-bg p-5">
        <div className="pointer-events-none absolute -right-16 -top-24 h-64 w-64 rounded-full bg-accent/20 blur-3xl" />
        <div className="relative flex items-start justify-between gap-6">
          <div className="min-w-0">
            <div className="mb-2 flex items-center gap-2">
              <LoaderBadge loader={instance.loader} />
              <span className="text-xs text-muted">{timeAgo(instance.lastPlayed)}</span>
            </div>
            <h1 className="truncate text-2xl font-bold tracking-tight">{instance.name}</h1>
            <p className="mt-1 text-sm text-muted">
              Minecraft {instance.minecraftVersion}
              {instance.loaderVersion && ` · ${instance.loader} ${instance.loaderVersion}`}
            </p>
          </div>
          <div className="flex shrink-0 gap-1">
            {offlineWithMods && (
              <Button variant="ghost" title="Importar skin (offline)" onClick={() => void importSkin()}>
                <Image size={16} />
              </Button>
            )}
            <Button variant="subtle" title="Editar perfil" disabled={isRunning || isPreparing} onClick={() => setEditing(true)}>
              <Settings2 size={16} /> Editar perfil
            </Button>
            <Button variant="ghost" title="Abrir pasta" onClick={() => void api.openInstanceFolder(instance.id)}>
              <FolderOpen size={16} />
            </Button>
            <Button variant="ghost" title="Excluir instância" disabled={isRunning || isPreparing} onClick={props.onDelete}>
              <Trash2 size={16} />
            </Button>
          </div>
        </div>

        <div className="relative mt-4 grid grid-cols-[1fr_auto] items-end gap-6">
          <div className="space-y-3">
            <div className="flex items-center justify-between text-sm">
              <span className="flex items-center gap-2 font-medium">
                <Cpu size={15} className="text-muted" /> Memória máxima
              </span>
              <span className="font-mono tabular-nums">
                {(memory / 1024).toFixed(1)} GB <span className="text-muted">/ {(props.totalMemoryMb / 1024).toFixed(0)} GB</span>
              </span>
            </div>
            <input
              type="range"
              className="memory w-full"
              min={1024}
              max={maxSlider}
              step={512}
              value={memory}
              disabled={isRunning || isPreparing}
              style={{ ["--fill" as string]: `${fill}%` }}
              onChange={(e) => setMemory(Number(e.target.value))}
              onPointerUp={() => memory !== instance.memoryMaxMb && commit({ memoryMaxMb: memory, memoryMinMb: Math.min(instance.memoryMinMb, memory) })}
              onKeyUp={() => memory !== instance.memoryMaxMb && commit({ memoryMaxMb: memory })}
            />
            <div className="flex gap-6">
              <Toggle
                checked={instance.optimizedGc}
                onChange={(v) => commit({ optimizedGc: v })}
                label={<span className="text-xs text-muted">G1GC otimizado (Aikar)</span>}
              />
              {instance.loader !== "vanilla" && (
                <Toggle
                  checked={instance.skinLoader}
                  onChange={(v) => commit({ skinLoader: v })}
                  label={<span className="text-xs text-muted">Skins offline (CSL)</span>}
                />
              )}
            </div>
          </div>

          <PlayButton
            disabled={!account}
            running={isRunning}
            preparing={isPreparing}
            onPlay={props.onPlay}
            onKill={props.onKill}
          />
        </div>

        {isPreparing && progress && (
          <div className="relative mt-5">
            <DownloadProgress progress={progress} speed={props.speed} />
          </div>
        )}
      </section>

      {exit?.crashed && (
        <div className="flex animate-pop-in items-start gap-3 rounded-xl border border-err/40 bg-err/10 px-4 py-3 text-sm">
          <AlertTriangle size={18} className="mt-0.5 shrink-0 text-err" />
          <div className="selectable min-w-0 flex-1">
            <p className="font-semibold text-red-100">O jogo fechou com erro (código {exit.code ?? "?"})</p>
            {exit.crashReport ? (
              <p className="truncate text-xs text-red-200/80">Crash report: {exit.crashReport}</p>
            ) : (
              <p className="text-xs text-red-200/80">Veja as linhas em vermelho no console abaixo.</p>
            )}
          </div>
          <button onClick={props.onDismissExit} className="text-xs text-red-200/70 hover:text-white">
            Dispensar
          </button>
        </div>
      )}

      <nav className="flex gap-1 border-b border-line">
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={cx(
              "-mb-px flex items-center gap-2 border-b-2 px-3.5 py-2 text-sm font-medium transition",
              tab === t.id ? "border-accent text-white" : "border-transparent text-muted hover:text-zinc-200",
            )}
          >
            <t.icon size={15} /> {t.label}
          </button>
        ))}
      </nav>

      <div className="min-h-0 flex-1">
        {tab === "console" && <Console lines={props.logs} onClear={props.onClearLogs} />}
        {tab === "mods" && <ModsTab instance={instance} locked={locked} notify={props.notify} refreshKey={contentVersion} />}
        {tab === "browse" && <BrowseTab instance={instance} locked={locked} notify={props.notify} onInstalled={bumpContent} />}
        {tab === "configs" && <ConfigsTab instance={instance} notify={props.notify} />}
        {tab === "optimize" && (
          <OptimizeTab
            instance={instance}
            locked={locked}
            totalMemoryMb={props.totalMemoryMb}
            notify={props.notify}
            onChange={props.onChange}
            onInstalled={bumpContent}
            onConvertToFabric={props.onConvertToFabric}
          />
        )}
      </div>

      <EditInstanceModal
        open={editing}
        instance={instance}
        totalMemoryMb={props.totalMemoryMb}
        onClose={() => setEditing(false)}
        onSave={props.onSave}
      />
    </div>
  );
}

function PlayButton({
  disabled,
  running,
  preparing,
  onPlay,
  onKill,
}: {
  disabled: boolean;
  running: boolean;
  preparing: boolean;
  onPlay: () => void;
  onKill: () => void;
}) {
  if (running) {
    return (
      <button
        onClick={onKill}
        className="flex h-14 w-48 items-center justify-center gap-2 rounded-xl bg-err/15 text-base font-bold text-err ring-1 ring-err/40 transition hover:bg-err hover:text-white active:scale-[0.97]"
      >
        <Square size={18} fill="currentColor" /> Fechar jogo
      </button>
    );
  }
  return (
    <button
      onClick={onPlay}
      disabled={disabled || preparing}
      title={disabled ? "Adicione uma conta para jogar" : undefined}
      className={cx(
        "group relative flex h-14 w-48 items-center justify-center gap-2 overflow-hidden rounded-xl text-lg font-bold text-white transition active:scale-[0.97]",
        "bg-gradient-to-r from-ok to-emerald-500 shadow-xl shadow-ok/25 hover:shadow-ok/40 hover:brightness-110",
        "disabled:cursor-not-allowed disabled:opacity-60 disabled:shadow-none disabled:hover:brightness-100",
      )}
    >
      <span className="absolute inset-0 -translate-x-full bg-gradient-to-r from-transparent via-white/25 to-transparent transition-transform duration-700 group-hover:translate-x-full" />
      {preparing ? (
        <>
          <Spinner size={18} /> Preparando…
        </>
      ) : (
        <>
          <Play size={20} fill="currentColor" /> Jogar
        </>
      )}
    </button>
  );
}
