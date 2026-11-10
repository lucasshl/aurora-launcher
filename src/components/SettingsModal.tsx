import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { RefreshCw } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import type { Settings, SystemInfo } from "../lib/types";
import { Button, Field, Modal, Spinner, Toggle, inputClass } from "./ui";

interface Props {
  open: boolean;
  onClose: () => void;
  settings: Settings | null;
  system: SystemInfo | null;
  onSaved: (s: Settings) => void;
  onCheckUpdates: () => void;
  checkingUpdates: boolean;
}

export function SettingsModal({
  open,
  onClose,
  settings,
  system,
  onSaved,
  onCheckUpdates,
  checkingUpdates,
}: Props) {
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => undefined);
  }, []);
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) setDraft(settings);
  }, [open, settings]);

  if (!draft) return null;
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) =>
    setDraft({ ...draft, [k]: v });

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      onSaved(
        await api.saveSettings({
          ...draft,
          msClientId: draft.msClientId?.trim() || null,
          customSkinApi: draft.customSkinApi?.trim() || null,
          cosmeticsServer: draft.cosmeticsServer?.trim() || null,
        }),
      );
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="Configurações" width="max-w-xl">
      <div className="space-y-5">
        {system?.builtInMsClientId ? (
          <p className="rounded-lg border border-line bg-panel px-3 py-2 text-xs text-muted">
            Login Microsoft configurado neste build do launcher.
          </p>
        ) : (
          <Field
            label="Client ID do Azure (login Microsoft)"
            hint="App registrado em portal.azure.com com contas pessoais da Microsoft e fluxo de cliente público habilitado."
          >
            <input
              className={inputClass + " font-mono"}
              value={draft.msClientId ?? ""}
              onChange={(e) => set("msClientId", e.target.value)}
              placeholder="00000000-0000-0000-0000-000000000000"
            />
          </Field>
        )}

        <Field
          label="API de skins customizada (CustomSkinAPI)"
          hint="Primeira opção da cascata para contas offline, antes de Ely.by e Mojang. Ex.: https://littleskin.cn/csl/"
        >
          <input
            className={inputClass}
            value={draft.customSkinApi ?? ""}
            onChange={(e) => set("customSkinApi", e.target.value)}
            placeholder="https://skins.seuservidor.com/csl/"
          />
        </Field>

        <Field
          label="Servidor de cosméticos"
          hint="URL do cosmetics-server (crate incluído no projeto). Todos que usarem o mesmo servidor veem as skins e capas uns dos outros."
        >
          <input
            className={inputClass}
            value={draft.cosmeticsServer ?? ""}
            onChange={(e) => set("cosmeticsServer", e.target.value)}
            placeholder="https://cosmeticos.seudominio.com"
          />
        </Field>

        <div className="grid grid-cols-2 gap-4">
          <Field label="Downloads simultâneos">
            <input
              type="number"
              min={1}
              max={128}
              className={inputClass}
              value={draft.downloadConcurrency}
              onChange={(e) =>
                set(
                  "downloadConcurrency",
                  Math.max(1, Math.min(128, Number(e.target.value) || 1)),
                )
              }
            />
          </Field>
          <Field label="RAM padrão de novas instâncias (MB)">
            <input
              type="number"
              min={1024}
              step={512}
              className={inputClass}
              value={draft.defaultMemoryMaxMb}
              onChange={(e) =>
                set(
                  "defaultMemoryMaxMb",
                  Math.max(1024, Number(e.target.value) || 1024),
                )
              }
            />
          </Field>
        </div>

        <Toggle
          checked={draft.keepConsoleAttached}
          onChange={(v) => set("keepConsoleAttached", v)}
          label={
            <span>
              Capturar logs no console
              <span className="block text-xs text-muted">
                Desligado: a JVM roda desacoplada do launcher.
              </span>
            </span>
          }
        />

        <div className="flex items-center justify-between rounded-lg bg-bg px-3 py-2.5">
          <span className="text-sm">
            Versão <b>{version || "…"}</b>
          </span>
          <Button
            variant="subtle"
            disabled={checkingUpdates}
            onClick={onCheckUpdates}
          >
            {checkingUpdates ? <Spinner size={14} /> : <RefreshCw size={14} />}{" "}
            Verificar atualizações
          </Button>
        </div>

        {system && (
          <p className="selectable truncate text-xs text-muted">
            Dados em {system.dataDir}
          </p>
        )}
        {error && <p className="text-sm text-err">{error}</p>}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button variant="primary" disabled={busy} onClick={() => void save()}>
            {busy && <Spinner />} Salvar
          </Button>
        </div>
      </div>
    </Modal>
  );
}
