import { useEffect, useMemo, useState } from "react";
import { api, errorMessage } from "../lib/api";
import type { GameVersion, InstanceConfig, ModLoader } from "../lib/types";
import { Button, Field, Modal, Spinner, Toggle, cx, inputClass } from "./ui";

const LOADERS: { id: ModLoader; label: string; note?: string }[] = [
  { id: "vanilla", label: "Vanilla" },
  { id: "fabric", label: "Fabric" },
  { id: "forge", label: "Forge", note: "requer instalador" },
  { id: "neoforge", label: "NeoForge", note: "requer instalador" },
];

interface Props {
  open: boolean;
  onClose: () => void;
  onCreated: (inst: InstanceConfig, optimize: boolean) => void;
}

const VERSION_KINDS: { id: string; label: string }[] = [
  { id: "release", label: "Releases" },
  { id: "snapshot", label: "Snapshots" },
  { id: "old_beta", label: "Beta" },
  { id: "old_alpha", label: "Alpha" },
];

export function CreateInstanceModal({ open, onClose, onCreated }: Props) {
  const [versions, setVersions] = useState<GameVersion[]>([]);
  const [kinds, setKinds] = useState<string[]>(["release"]);
  const [versionFilter, setVersionFilter] = useState("");
  const [optimize, setOptimize] = useState(true);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [name, setName] = useState("");
  const [version, setVersion] = useState("");
  const [loader, setLoader] = useState<ModLoader>("fabric");
  const [loaderVersions, setLoaderVersions] = useState<string[]>([]);
  const [loaderVersion, setLoaderVersion] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setLoadingVersions(true);
    api
      .listMinecraftVersions(kinds)
      .then((v) => {
        setVersions(v);
        setVersion((cur) => (v.some((x) => x.id === cur) ? cur : v[0]?.id || ""));
      })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoadingVersions(false));
  }, [open, kinds]);

  useEffect(() => {
    setLoaderVersions([]);
    setLoaderVersion("");
    if (loader !== "fabric" || !version) return;
    api
      .listFabricVersions(version)
      .then(setLoaderVersions)
      .catch(() => setLoaderVersions([]));
  }, [loader, version]);

  const fabricUnsupported = loader === "fabric" && version !== "" && loaderVersions.length === 0;
  const suggestedName = useMemo(
    () => `${version}${loader !== "vanilla" ? ` ${LOADERS.find((l) => l.id === loader)?.label}` : ""}`,
    [version, loader],
  );

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const inst = await api.createInstance({
        name: name.trim() || suggestedName,
        minecraftVersion: version,
        loader,
        loaderVersion: loaderVersion || null,
      });
      onCreated(inst, optimize && loader !== "vanilla");
      onClose();
      setName("");
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="Nova instância" width="max-w-xl">
      <div className="space-y-5">
        <Field label="Nome">
          <input className={inputClass} value={name} placeholder={suggestedName} onChange={(e) => setName(e.target.value)} />
        </Field>

        <Field label={`Versão do Minecraft (${versions.length} disponíveis)`}>
          <div className="mb-2 flex flex-wrap gap-1.5">
            {VERSION_KINDS.map((k) => {
              const on = kinds.includes(k.id);
              return (
                <button
                  key={k.id}
                  type="button"
                  onClick={() => setKinds((cur) => (on ? (cur.length > 1 ? cur.filter((x) => x !== k.id) : cur) : [...cur, k.id]))}
                  className={cx(
                    "rounded-full border px-3 py-1 text-xs font-medium transition",
                    on ? "border-accent bg-accent/15 text-white" : "border-line text-muted hover:text-zinc-200",
                  )}
                >
                  {k.label}
                </button>
              );
            })}
            <input
              value={versionFilter}
              onChange={(e) => setVersionFilter(e.target.value)}
              placeholder="Filtrar (ex.: 1.8)"
              className="ml-auto w-32 rounded-md border border-line bg-bg px-2 py-1 text-xs outline-none focus:border-accent"
            />
          </div>
          <select
            className={inputClass}
            value={version}
            disabled={loadingVersions}
            size={6}
            onChange={(e) => setVersion(e.target.value)}
          >
            {versions
              .filter((v) => !versionFilter || v.id.includes(versionFilter))
              .map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id}
                  {v.kind !== "release" ? `  ·  ${v.kind.replace("old_", "")}` : ""}
                  {"  ·  "}
                  {v.releaseTime.slice(0, 10)}
                </option>
              ))}
          </select>
        </Field>

        <Field label="Modloader">
          <div className="grid grid-cols-4 gap-2">
            {LOADERS.map((l) => (
              <button
                key={l.id}
                type="button"
                onClick={() => setLoader(l.id)}
                className={cx(
                  "rounded-lg border px-2 py-2.5 text-sm font-medium transition",
                  loader === l.id ? "border-accent bg-accent/15 text-white" : "border-line bg-bg text-muted hover:text-zinc-200",
                )}
              >
                {l.label}
                {l.note && <span className="block text-[10px] font-normal opacity-70">{l.note}</span>}
              </button>
            ))}
          </div>
        </Field>

        {loader === "fabric" && loaderVersions.length > 0 && (
          <Field label="Versão do Fabric Loader" hint="Em branco = mais recente estável">
            <select className={inputClass} value={loaderVersion} onChange={(e) => setLoaderVersion(e.target.value)}>
              <option value="">Mais recente estável</option>
              {loaderVersions.slice(0, 40).map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
          </Field>
        )}
        {fabricUnsupported && <p className="text-sm text-warn">O Fabric ainda não suporta esta versão.</p>}
        {(loader === "forge" || loader === "neoforge") && (
          <p className="rounded-md bg-bg px-3 py-2 text-xs text-muted">
            Forge e NeoForge precisam rodar o instalador oficial (ele aplica patches no client). Depois, informe o ID
            gerado em <code>versionId</code> nas opções da instância.
          </p>
        )}

        {loader !== "vanilla" && (
          <Toggle
            checked={optimize}
            onChange={setOptimize}
            label={
              <span>
                Otimizar automaticamente
                <span className="block text-xs text-muted">Instala o pacote de desempenho (Sodium, Lithium, C2ME…) e o preset para PC fraco.</span>
              </span>
            }
          />
        )}

        {error && <p className="selectable text-sm text-err">{error}</p>}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={!version || busy || fabricUnsupported} onClick={() => void submit()}>
            {busy && <Spinner />} Criar instância
          </Button>
        </div>
      </div>
    </Modal>
  );
}
