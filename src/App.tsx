import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Box } from "lucide-react";
import { AccountSwitcher } from "./components/AccountSwitcher";
import { AddAccountModal } from "./components/AddAccountModal";
import { CreateInstanceModal } from "./components/CreateInstanceModal";
import { InstancePanel } from "./components/InstancePanel";
import { InstanceSidebar } from "./components/InstanceSidebar";
import { SettingsModal } from "./components/SettingsModal";
import { Button, Toasts, type Toast } from "./components/ui";
import { UpdateDialog, useUpdater } from "./components/UpdateDialog";
import { useLaunchEvents } from "./hooks/useLaunchEvents";
import { api, errorMessage } from "./lib/api";
import type { AccountsView, Cosmetic, ExitInfo, InstanceConfig, Settings, SystemInfo, Wardrobe } from "./lib/types";

// three.js (preview 3D) só é carregado ao abrir o guarda-roupa
const WardrobePage = lazy(() => import("./components/WardrobePage").then((m) => ({ default: m.WardrobePage })));
const SkinEditor = lazy(() => import("./components/SkinEditor").then((m) => ({ default: m.SkinEditor })));

let toastSeq = 0;

export default function App() {
  const [accounts, setAccounts] = useState<AccountsView>({ active: null, accounts: [] });
  const [instances, setInstances] = useState<InstanceConfig[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [running, setRunning] = useState<Set<string>>(new Set());
  const [preparing, setPreparing] = useState<Set<string>>(new Set());
  const [modal, setModal] = useState<"account" | "instance" | "settings" | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [view, setView] = useState<"instances" | "wardrobe" | "editor">("instances");
  const [accountTab, setAccountTab] = useState<"microsoft" | "offline">("offline");
  const [wardrobe, setWardrobe] = useState<Wardrobe>({ items: [], equipped: {} });
  const [textures, setTextures] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<Cosmetic | null>(null);
  const texturesRef = useRef(textures);
  texturesRef.current = textures;

  /** Recarrega o guarda-roupa e busca as texturas (data URL) que ainda não estão em cache. */
  const refreshWardrobe = useCallback(async (w?: Wardrobe) => {
    const next = w ?? (await api.getWardrobe());
    setWardrobe(next);
    const missing = next.items.filter((c) => !texturesRef.current[c.id]);
    if (missing.length) {
      const urls = await Promise.all(missing.map((c) => api.cosmeticDataUrl(c.id).catch(() => "")));
      setTextures((t) => ({ ...t, ...Object.fromEntries(missing.map((c, i) => [c.id, urls[i] ?? ""])) }));
    }
  }, []);
  const ev = useLaunchEvents();

  const notify = useCallback((kind: Toast["kind"], text: string) => {
    const id = ++toastSeq;
    setToasts((t) => [...t, { id, kind, text }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 9000 : 4000);
  }, []);

  const updater = useUpdater(notify);

  // Carga inicial
  useEffect(() => {
    void (async () => {
      try {
        const [acc, inst, set, sys, run] = await Promise.all([
          api.listAccounts(),
          api.listInstances(),
          api.getSettings(),
          api.systemInfo(),
          api.runningInstances(),
        ]);
        setAccounts(acc);
        setInstances(inst);
        setSelectedId(inst[0]?.id ?? null);
        setSettings(set);
        setSystem(sys);
        setRunning(new Set(run));
        void refreshWardrobe();
      } catch (e) {
        notify("error", `Falha ao carregar o launcher: ${errorMessage(e)}`);
      }
    })();
  }, [notify]);

  // Jogo fechou → atualiza estado e avisa em caso de crash (sem depender de já estar marcado como aberto)
  const seenExits = useRef<Set<ExitInfo>>(new Set());
  useEffect(() => {
    for (const [id, info] of Object.entries(ev.exits)) {
      if (!info || seenExits.current.has(info)) continue;
      seenExits.current.add(info);
      setRunning((r) => {
        if (!r.has(id)) return r;
        const n = new Set(r);
        n.delete(id);
        return n;
      });
      if (info.crashed) notify("error", "O Minecraft fechou com erro. Veja o console (linhas em vermelho).");
    }
  }, [ev.exits, notify]);

  const selected = useMemo(() => instances.find((i) => i.id === selectedId) ?? null, [instances, selectedId]);
  const activeAccount = useMemo(
    () => accounts.accounts.find((a) => a.id === accounts.active) ?? null,
    [accounts],
  );

  const play = async (inst: InstanceConfig) => {
    if (!activeAccount) {
      setModal("account");
      return;
    }
    ev.clearExit(inst.id);
    ev.clearLogs(inst.id);
    setPreparing((p) => new Set(p).add(inst.id));
    const startedAt = Date.now();
    try {
      await api.launch(inst.id, activeAccount.id);
      // Se o jogo já fechou enquanto o comando retornava, não marca como aberto
      if (!ev.exitedSince(inst.id, startedAt)) {
        setRunning((r) => new Set(r).add(inst.id));
        // Confirma com o backend logo depois (cobre o caso de o evento chegar atrasado)
        setTimeout(() => {
          void api.runningInstances().then((ids) => {
            if (!ids.includes(inst.id)) {
              setRunning((r) => {
                const n = new Set(r);
                n.delete(inst.id);
                return n;
              });
            }
          });
        }, 1500);
      }
      setInstances(await api.listInstances());
    } catch (e) {
      const msg = errorMessage(e);
      ev.pushLocalLog({ instanceId: inst.id, stream: "launcher", level: "error", line: msg });
      notify("error", msg);
    } finally {
      setPreparing((p) => {
        const n = new Set(p);
        n.delete(inst.id);
        return n;
      });
      ev.clearProgress(inst.id);
    }
  };

  /** Pacote de desempenho recomendado + preset para PC fraco, logo após criar a instância. */
  const autoOptimize = async (inst: InstanceConfig) => {
    notify("info", `Otimizando ${inst.name}: verificando mods compatíveis…`);
    try {
      const list = await api.curatedMods(inst.id, "performance");
      const keys = list.filter((m) => m.recommended && m.availableSlug).map((m) => m.key);
      const report = await api.installCurated(inst.id, keys);
      await api.applyGraphicsPreset(inst.id, "potato");
      notify("success", `${inst.name} otimizada: ${report.installed.length} mods instalados e preset para PC fraco aplicado.`);
    } catch (e) {
      notify("error", `Otimização automática falhou: ${errorMessage(e)}`);
    }
  };

  /** Salva o perfil e lança erro (o menu de perfil mostra a mensagem). */
  const saveInstance = async (cfg: InstanceConfig) => {
    const saved = await api.updateInstance(cfg);
    setInstances((list) => list.map((i) => (i.id === saved.id ? saved : i)));
    notify("success", `Perfil "${saved.name}" salvo.`);
  };

  /** Vanilla → Fabric na mesma versão + pacote de desempenho + preset PC fraco. */
  const convertToFabric = async (inst: InstanceConfig) => {
    try {
      const loaders = await api.listFabricVersions(inst.minecraftVersion);
      if (loaders.length === 0) {
        notify("error", `O Fabric ainda não suporta o Minecraft ${inst.minecraftVersion}.`);
        return;
      }
      const saved = await api.updateInstance({ ...inst, loader: "fabric", loaderVersion: null, versionId: null });
      setInstances((list) => list.map((i) => (i.id === saved.id ? saved : i)));
      await autoOptimize(saved);
    } catch (e) {
      notify("error", errorMessage(e));
    }
  };

  const updateInstance = async (cfg: InstanceConfig) => {
    setInstances((list) => list.map((i) => (i.id === cfg.id ? cfg : i)));
    try {
      const saved = await api.updateInstance(cfg);
      setInstances((list) => list.map((i) => (i.id === saved.id ? saved : i)));
    } catch (e) {
      notify("error", errorMessage(e));
      setInstances(await api.listInstances());
    }
  };

  const deleteInstance = async (inst: InstanceConfig) => {
    if (!window.confirm(`Excluir "${inst.name}"? Mundos, mods e configurações desta instância serão apagados.`)) return;
    try {
      await api.deleteInstance(inst.id);
      const list = await api.listInstances();
      setInstances(list);
      setSelectedId(list[0]?.id ?? null);
    } catch (e) {
      notify("error", errorMessage(e));
    }
  };

  const withAccounts = async (fn: () => Promise<AccountsView>) => {
    try {
      setAccounts(await fn());
    } catch (e) {
      notify("error", errorMessage(e));
    }
  };

  return (
    <div className="flex h-full">
      <InstanceSidebar
        instances={instances}
        selected={selectedId}
        running={running}
        preparing={preparing}
        onSelect={(id) => {
          setSelectedId(id);
          setView("instances");
        }}
        onCreate={() => setModal("instance")}
        onSettings={() => setModal("settings")}
        onWardrobe={() => setView("wardrobe")}
        wardrobeActive={view !== "instances"}
      />

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="relative z-30 flex h-14 shrink-0 items-center justify-end border-b border-line bg-panel px-5">
          <AccountSwitcher
            view={accounts}
            wardrobe={wardrobe}
            textures={textures}
            onAdd={(tab) => {
              setAccountTab(tab);
              setModal("account");
            }}
            onSelect={(id) => void withAccounts(() => api.setActiveAccount(id))}
            onRemove={(id) => void withAccounts(() => api.removeAccount(id))}
            onEquipSkin={(id) =>
              activeAccount &&
              void api
                .equipCosmetic(activeAccount.id, "skin", id)
                .then((w) => refreshWardrobe(w))
                .catch((e) => notify("error", errorMessage(e)))
            }
            onOpenWardrobe={() => setView("wardrobe")}
            onOpenEditor={() => {
              const id = activeAccount ? wardrobe.equipped[activeAccount.id]?.skin : undefined;
              setEditing(wardrobe.items.find((c) => c.id === id) ?? null);
              setView("editor");
            }}
          />
        </header>

        <div className="min-h-0 flex-1">
          {view === "editor" ? (
            <Suspense fallback={<div className="grid h-full place-items-center text-sm text-muted">Carregando…</div>}>
              <SkinEditor
                key={editing?.id ?? "novo"}
                source={editing}
                sourceTexture={editing ? textures[editing.id] : undefined}
                notify={notify}
                onClose={() => setView("wardrobe")}
                onSaved={async (c, equip) => {
                  if (equip && activeAccount) await refreshWardrobe(await api.equipCosmetic(activeAccount.id, "skin", c.id));
                  else await refreshWardrobe();
                  setEditing(c);
                }}
              />
            </Suspense>
          ) : view === "wardrobe" ? (
            <Suspense fallback={<div className="grid h-full place-items-center text-sm text-muted">Carregando…</div>}>
              <WardrobePage
                account={activeAccount}
                settings={settings}
                wardrobe={wardrobe}
                textures={textures}
                refresh={refreshWardrobe}
                notify={notify}
                onEdit={(c) => {
                  setEditing(c);
                  setView("editor");
                }}
              />
            </Suspense>
          ) : selected ? (
            <InstancePanel
              instance={selected}
              account={activeAccount}
              totalMemoryMb={system?.totalMemoryMb ?? 8192}
              isRunning={running.has(selected.id)}
              isPreparing={preparing.has(selected.id)}
              progress={ev.progress[selected.id]}
              speed={ev.speed[selected.id] ?? 0}
              logs={ev.logs[selected.id] ?? []}
              exit={ev.exits[selected.id]}
              onPlay={() => void play(selected)}
              onKill={() => void api.kill(selected.id)}
              onChange={(cfg) => void updateInstance(cfg)}
              onSave={saveInstance}
              onConvertToFabric={() => convertToFabric(selected)}
              onDelete={() => void deleteInstance(selected)}
              onClearLogs={() => ev.clearLogs(selected.id)}
              onDismissExit={() => ev.clearExit(selected.id)}
              notify={notify}
            />
          ) : (
            <div className="grid h-full place-items-center">
              <div className="max-w-sm text-center">
                <div className="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-panel-2">
                  <Box size={28} className="text-muted" />
                </div>
                <h2 className="text-xl font-semibold">Nenhuma instância ainda</h2>
                <p className="mt-2 text-sm text-muted">
                  Cada instância tem seus próprios mods, mundos e configurações. Bibliotecas e assets são compartilhados.
                </p>
                <Button variant="primary" className="mt-5" onClick={() => setModal("instance")}>
                  Criar instância
                </Button>
              </div>
            </div>
          )}
        </div>
      </main>

      <AddAccountModal
        initialTab={accountTab}
        open={modal === "account"}
        onClose={() => setModal(null)}
        hasClientId={!!(settings?.msClientId || system?.hasMsClientId)}
        onOpenSettings={() => setModal("settings")}
        onAdded={(acc) => {
          notify("success", `Conta ${acc.username} adicionada.`);
          void api.listAccounts().then(setAccounts);
        }}
      />
      <CreateInstanceModal
        open={modal === "instance"}
        onClose={() => setModal(null)}
        onCreated={(inst, optimize) => {
          setInstances((l) => [inst, ...l]);
          setSelectedId(inst.id);
          setView("instances");
          if (optimize) void autoOptimize(inst);
        }}
      />
      <SettingsModal
        open={modal === "settings"}
        onClose={() => setModal(null)}
        settings={settings}
        system={system}
        onCheckUpdates={() => void updater.checkNow(true)}
        checkingUpdates={updater.checking}
        onSaved={(s) => {
          setSettings(s);
          void api.systemInfo().then(setSystem);
          notify("success", "Configurações salvas.");
        }}
      />
      <UpdateDialog update={updater.update} onClose={updater.dismiss} />
      <Toasts toasts={toasts} dismiss={(id) => setToasts((t) => t.filter((x) => x.id !== id))} />
    </div>
  );
}
