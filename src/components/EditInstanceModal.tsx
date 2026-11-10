import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderSearch, RotateCcw } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import type { GameVersion, InstanceConfig, ModLoader } from "../lib/types";
import { Button, Field, Modal, Spinner, Toggle, cx, inputClass } from "./ui";

const LOADERS: { id: ModLoader; label: string }[] = [
  { id: "vanilla", label: "Vanilla" },
  { id: "fabric", label: "Fabric" },
  { id: "forge", label: "Forge" },
  { id: "neoforge", label: "NeoForge" },
];

type Tab = "geral" | "java" | "janela";

interface Props {
  open: boolean;
  instance: InstanceConfig;
  totalMemoryMb: number;
  onClose: () => void;
  onSave: (cfg: InstanceConfig) => Promise<void>;
}

/** Menu de perfil da instância: tudo que fica no instance.json, editável pela UI. */
export function EditInstanceModal({ open, instance, totalMemoryMb, onClose, onSave }: Props) {
  const [draft, setDraft] = useState<InstanceConfig>(instance);
  const [tab, setTab] = useState<Tab>("geral");
  const [versions, setVersions] = useState<GameVersion[]>([]);
  const [showAll, setShowAll] = useState(false);
  const [fabricVersions, setFabricVersions] = useState<string[]>([]);
  const [jvmText, setJvmText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setDraft(instance);
    setJvmText(instance.jvmArgs.join("\n"));
    setError(null);
    setTab("geral");
  }, [open, instance]);

  useEffect(() => {
    if (!open) return;
    api
      .listMinecraftVersions(showAll ? [] : ["release"])
      .then(setVersions)
      .catch(() => setVersions([]));
  }, [open, showAll]);

  useEffect(() => {
    setFabricVersions([]);
    if (!open || draft.loader !== "fabric") return;
    api.listFabricVersions(draft.minecraftVersion).then(setFabricVersions).catch(() => undefined);
  }, [open, draft.loader, draft.minecraftVersion]);

  const set = <K extends keyof InstanceConfig>(k: K, v: InstanceConfig[K]) => setDraft((d) => ({ ...d, [k]: v }));
  const maxRam = Math.max(2048, Math.floor((totalMemoryMb * 0.85) / 512) * 512);
  const versionChanged = draft.minecraftVersion !== instance.minecraftVersion || draft.loader !== instance.loader;

  const versionOptions = useMemo(() => {
    const list = versions.map((v) => v.id);
    return list.includes(draft.minecraftVersion) ? list : [draft.minecraftVersion, ...list];
  }, [versions, draft.minecraftVersion]);

  const pickJava = async () => {
    const file = await openDialog({ multiple: false, filters: [{ name: "Java", extensions: ["exe", ""] }] });
    if (typeof file === "string") set("javaPath", file);
  };

  const save = async () => {
    if (!draft.name.trim()) {
      setError("Dê um nome ao perfil.");
      return;
    }
    if (draft.memoryMinMb > draft.memoryMaxMb) {
      setError("A RAM mínima não pode ser maior que a máxima.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await onSave({
        ...draft,
        name: draft.name.trim(),
        javaPath: draft.javaPath?.trim() || null,
        jvmArgs: jvmText
          .split("\n")
          .map((l) => l.trim())
          .filter(Boolean),
      });
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="Editar perfil" width="max-w-2xl">
      <div className="mb-4 flex gap-1 border-b border-line">
        {(
          [
            ["geral", "Geral"],
            ["java", "Java e memória"],
            ["janela", "Janela"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={cx(
              "-mb-px border-b-2 px-3 py-2 text-sm font-medium transition",
              tab === id ? "border-accent text-white" : "border-transparent text-muted hover:text-zinc-200",
            )}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="min-h-72 space-y-4">
        {tab === "geral" && (
          <>
            <Field label="Nome do perfil">
              <input className={inputClass} value={draft.name} onChange={(e) => set("name", e.target.value)} />
            </Field>

            <div className="grid grid-cols-2 gap-3">
              <Field label="Versão do Minecraft">
                <select
                  className={inputClass}
                  value={draft.minecraftVersion}
                  onChange={(e) => setDraft((d) => ({ ...d, minecraftVersion: e.target.value, loaderVersion: null }))}
                >
                  {versionOptions.map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </Field>
              <div className="pt-6">
                <Toggle checked={showAll} onChange={setShowAll} label={<span className="text-xs text-muted">Mostrar snapshots/beta/alpha</span>} />
              </div>
            </div>

            <Field label="Modloader">
              <div className="grid grid-cols-4 gap-2">
                {LOADERS.map((l) => (
                  <button
                    key={l.id}
                    type="button"
                    onClick={() => setDraft((d) => ({ ...d, loader: l.id, loaderVersion: null }))}
                    className={cx(
                      "rounded-lg border px-2 py-2 text-sm font-medium transition",
                      draft.loader === l.id ? "border-accent bg-accent/15 text-white" : "border-line bg-bg text-muted hover:text-zinc-200",
                    )}
                  >
                    {l.label}
                  </button>
                ))}
              </div>
            </Field>

            {draft.loader === "fabric" && (
              <Field label="Versão do Fabric Loader">
                <select
                  className={inputClass}
                  value={draft.loaderVersion ?? ""}
                  onChange={(e) => set("loaderVersion", e.target.value || null)}
                >
                  <option value="">Mais recente estável</option>
                  {fabricVersions.slice(0, 40).map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </Field>
            )}

            {versionChanged && (
              <p className="rounded-md bg-warn/10 px-3 py-2 text-xs text-amber-100">
                Ao trocar versão ou modloader, os arquivos do jogo serão baixados de novo no próximo Jogar. Mods de outra versão
                podem não funcionar: confira a aba Mods.
              </p>
            )}

            <div className="space-y-1 border-t border-line pt-3">
              <Toggle checked={draft.optimizedGc} onChange={(v) => set("optimizedGc", v)} label="G1GC otimizado (Aikar's Flags)" />
              {draft.loader !== "vanilla" && (
                <Toggle checked={draft.skinLoader} onChange={(v) => set("skinLoader", v)} label="Skins para contas offline (CustomSkinLoader)" />
              )}
            </div>
          </>
        )}

        {tab === "java" && (
          <>
            <div className="grid grid-cols-2 gap-3">
              <Field label="RAM mínima (MB)">
                <input
                  type="number"
                  step={256}
                  min={256}
                  className={inputClass}
                  value={draft.memoryMinMb}
                  onChange={(e) => set("memoryMinMb", Math.max(256, Number(e.target.value) || 256))}
                />
              </Field>
              <Field label={`RAM máxima (MB) · recomendado até ${maxRam}`}>
                <input
                  type="number"
                  step={512}
                  min={1024}
                  className={cx(inputClass, draft.memoryMaxMb > maxRam && "border-warn")}
                  value={draft.memoryMaxMb}
                  onChange={(e) => set("memoryMaxMb", Math.max(1024, Number(e.target.value) || 1024))}
                />
              </Field>
            </div>
            <p className="text-xs text-muted">
              Para Minecraft com poucos mods, 2–4 GB bastam. RAM demais deixa o Windows sem memória e causa travadas.
            </p>

            <Field label="Java" hint="Em branco = o launcher escolhe e baixa a versão certa automaticamente.">
              <div className="flex gap-2">
                <input
                  className={inputClass + " font-mono text-xs"}
                  value={draft.javaPath ?? ""}
                  placeholder="Automático"
                  onChange={(e) => set("javaPath", e.target.value || null)}
                />
                <Button variant="subtle" title="Escolher java.exe" onClick={() => void pickJava()}>
                  <FolderSearch size={15} />
                </Button>
                {draft.javaPath && (
                  <Button variant="ghost" title="Voltar ao automático" onClick={() => set("javaPath", null)}>
                    <RotateCcw size={15} />
                  </Button>
                )}
              </div>
            </Field>

            <Field label="Argumentos extras da JVM (um por linha)" hint="Ex.: -Dfabric.log.level=debug">
              <textarea
                rows={5}
                spellCheck={false}
                className={inputClass + " resize-none font-mono text-xs"}
                value={jvmText}
                onChange={(e) => setJvmText(e.target.value)}
              />
            </Field>
          </>
        )}

        {tab === "janela" && (
          <>
            <Toggle
              checked={!!draft.resolution}
              onChange={(v) => set("resolution", v ? { width: 1280, height: 720 } : null)}
              label="Definir tamanho da janela ao abrir"
            />
            {draft.resolution && (
              <div className="grid grid-cols-2 gap-3">
                <Field label="Largura">
                  <input
                    type="number"
                    className={inputClass}
                    value={draft.resolution.width}
                    onChange={(e) => set("resolution", { ...draft.resolution!, width: Math.max(320, Number(e.target.value) || 854) })}
                  />
                </Field>
                <Field label="Altura">
                  <input
                    type="number"
                    className={inputClass}
                    value={draft.resolution.height}
                    onChange={(e) => set("resolution", { ...draft.resolution!, height: Math.max(240, Number(e.target.value) || 480) })}
                  />
                </Field>
              </div>
            )}
            <div className="flex flex-wrap gap-2">
              {[
                [854, 480],
                [1280, 720],
                [1600, 900],
                [1920, 1080],
              ].map(([w, h]) => (
                <button
                  key={w}
                  onClick={() => set("resolution", { width: w!, height: h! })}
                  className="rounded-md border border-line bg-bg px-2.5 py-1 text-xs text-muted transition hover:border-accent hover:text-white"
                >
                  {w}×{h}
                </button>
              ))}
            </div>
          </>
        )}
      </div>

      {error && <p className="selectable mt-3 text-sm text-err">{error}</p>}
      <div className="mt-5 flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose}>
          Cancelar
        </Button>
        <Button variant="primary" disabled={busy} onClick={() => void save()}>
          {busy && <Spinner size={14} />} Salvar perfil
        </Button>
      </div>
    </Modal>
  );
}
