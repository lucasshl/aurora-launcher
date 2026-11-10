import { useCallback, useEffect, useMemo, useState } from "react";
import { ArrowUpCircle, FolderOpen, Package, RefreshCw, Search, Trash2 } from "lucide-react";
import { api, errorMessage, formatBytes } from "../lib/api";
import type { ContentKind, InstalledContent, InstanceConfig, UpdateInfo } from "../lib/types";
import { Button, Spinner, cx } from "./ui";

export const KIND_LABEL: Record<ContentKind, string> = {
  mod: "Mods",
  resourcepack: "Resource packs",
  shader: "Shaders",
};

export function KindSwitch({ kind, onChange, allowMods }: { kind: ContentKind; onChange: (k: ContentKind) => void; allowMods: boolean }) {
  return (
    <div className="flex gap-1 rounded-lg bg-bg p-1">
      {(Object.keys(KIND_LABEL) as ContentKind[]).map((k) => (
        <button
          key={k}
          disabled={k === "mod" && !allowMods}
          onClick={() => onChange(k)}
          className={cx(
            "rounded-md px-3 py-1.5 text-xs font-semibold transition disabled:opacity-40",
            kind === k ? "bg-panel-3 text-white" : "text-muted hover:text-zinc-200",
          )}
        >
          {KIND_LABEL[k]}
        </button>
      ))}
    </div>
  );
}

interface Props {
  instance: InstanceConfig;
  locked: boolean;
  notify: (kind: "error" | "success" | "info", text: string) => void;
  refreshKey: number;
}

export function ModsTab({ instance, locked, notify, refreshKey }: Props) {
  const allowMods = instance.loader !== "vanilla";
  const [kind, setKind] = useState<ContentKind>(allowMods ? "mod" : "resourcepack");
  const [items, setItems] = useState<InstalledContent[]>([]);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState("");
  const [updates, setUpdates] = useState<UpdateInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.listContent(instance.id, kind));
    } catch (e) {
      notify("error", errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [instance.id, kind, notify]);

  useEffect(() => {
    setUpdates(null);
    void load();
  }, [load, refreshKey]);

  useEffect(() => {
    if (!allowMods && kind === "mod") setKind("resourcepack");
  }, [allowMods, kind]);

  const visible = useMemo(() => {
    const q = filter.toLowerCase();
    return items.filter((i) => !q || (i.meta.name ?? i.fileName).toLowerCase().includes(q) || i.fileName.toLowerCase().includes(q));
  }, [items, filter]);

  const updateMap = useMemo(() => new Map((updates ?? []).map((u) => [u.fileName, u])), [updates]);

  const run = async (label: string, fn: () => Promise<unknown>) => {
    setBusy(label);
    try {
      await fn();
    } catch (e) {
      notify("error", errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const checkUpdates = () =>
    run("updates", async () => {
      const u = await api.checkContentUpdates(instance.id, kind);
      setUpdates(u);
      notify("info", u.length ? `${u.length} atualização(ões) disponível(is).` : "Tudo atualizado.");
    });

  const updateAll = (names: string[]) =>
    run("apply", async () => {
      const n = await api.updateContent(instance.id, kind, names);
      notify("success", `${n} item(ns) atualizado(s).`);
      setUpdates(null);
      await load();
    });

  const enabledCount = items.filter((i) => i.enabled).length;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <KindSwitch kind={kind} onChange={setKind} allowMods={allowMods} />
        <div className="relative ml-auto">
          <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" />
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filtrar instalados"
            className="w-52 rounded-md border border-line bg-bg py-1.5 pl-8 pr-2 text-sm outline-none focus:border-accent"
          />
        </div>
        <Button variant="ghost" title="Abrir pasta" onClick={() => void api.openContentFolder(instance.id, kind)}>
          <FolderOpen size={15} />
        </Button>
        <Button variant="ghost" title="Recarregar" onClick={() => void load()}>
          <RefreshCw size={15} className={cx(loading && "animate-spin")} />
        </Button>
        <Button variant="subtle" disabled={busy !== null} onClick={() => void checkUpdates()}>
          {busy === "updates" ? <Spinner size={14} /> : <ArrowUpCircle size={15} />} Verificar atualizações
        </Button>
        {updates && updates.length > 0 && (
          <Button variant="primary" disabled={busy !== null || locked} onClick={() => void updateAll([])}>
            {busy === "apply" && <Spinner size={14} />} Atualizar tudo ({updates.length})
          </Button>
        )}
      </div>

      <p className="text-xs text-muted">
        {items.length} {KIND_LABEL[kind].toLowerCase()} · {enabledCount} ativos
        {locked && " · feche o jogo para alterar"}
      </p>

      <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-line bg-panel">
        {visible.length === 0 ? (
          <div className="grid h-full place-items-center p-10 text-center text-sm text-muted">
            <div>
              <Package size={28} className="mx-auto mb-2 opacity-60" />
              {loading ? "Carregando…" : "Nada instalado. Use a aba Explorar ou Otimização."}
            </div>
          </div>
        ) : (
          <ul className="divide-y divide-line">
            {visible.map((item) => {
              const upd = updateMap.get(item.fileName);
              return (
                <li key={item.fileName} className={cx("flex items-center gap-3 px-4 py-2.5 transition", !item.enabled && "opacity-50")}>
                  {item.meta.icon ? (
                    <img src={item.meta.icon} className="h-9 w-9 rounded-md bg-panel-3 object-cover" style={{ imageRendering: "pixelated" }} />
                  ) : (
                    <div className="grid h-9 w-9 place-items-center rounded-md bg-panel-3 text-muted">
                      <Package size={16} />
                    </div>
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">
                      {item.meta.name ?? item.fileName}
                      {item.meta.version && <span className="ml-2 text-xs font-normal text-muted">{item.meta.version}</span>}
                    </p>
                    <p className="truncate text-xs text-muted">
                      {item.meta.description ?? item.fileName} · {formatBytes(item.size)}
                    </p>
                  </div>
                  {upd && (
                    <button
                      disabled={busy !== null || locked}
                      onClick={() => void updateAll([item.fileName])}
                      className="rounded bg-ok/15 px-2 py-1 text-[11px] font-semibold text-ok transition hover:bg-ok hover:text-white"
                    >
                      → {upd.latest.versionNumber}
                    </button>
                  )}
                  <Switch
                    checked={item.enabled}
                    disabled={locked}
                    onChange={(v) =>
                      void run("toggle", async () => {
                        await api.toggleContent(instance.id, kind, item.fileName, v);
                        setItems((list) => list.map((x) => (x.fileName === item.fileName ? { ...x, enabled: v } : x)));
                      })
                    }
                  />
                  <button
                    title="Remover"
                    disabled={locked}
                    onClick={() =>
                      window.confirm(`Remover ${item.meta.name ?? item.fileName}?`) &&
                      void run("remove", async () => {
                        await api.removeContent(instance.id, kind, item.fileName);
                        setItems((list) => list.filter((x) => x.fileName !== item.fileName));
                      })
                    }
                    className="rounded p-1.5 text-muted transition hover:bg-err/15 hover:text-err disabled:opacity-40"
                  >
                    <Trash2 size={15} />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}

function Switch({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx("relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-40", checked ? "bg-ok" : "bg-panel-3")}
    >
      <span className={cx("absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform", checked ? "translate-x-4.5" : "translate-x-0.5")} />
    </button>
  );
}
