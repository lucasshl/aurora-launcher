import { useCallback, useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, Rocket } from "lucide-react";
import { errorMessage, formatBytes } from "../lib/api";
import { Button, Modal, Spinner } from "./ui";

type Phase = "idle" | "available" | "downloading" | "installing" | "error";

const SKIP_KEY = "aurora.skipUpdate";

/**
 * Verifica o latest.json do GitHub Releases na inicialização. Se a versão publicada for
 * maior que a instalada, pergunta ao usuário; ao confirmar, baixa o instalador assinado,
 * instala e reinicia o launcher. A assinatura é conferida com a chave pública embutida.
 */
export function useUpdater(notify: (kind: "error" | "success" | "info", text: string) => void) {
  const [update, setUpdate] = useState<Update | null>(null);
  const [checking, setChecking] = useState(false);

  const checkNow = useCallback(
    async (manual: boolean) => {
      setChecking(true);
      try {
        const u = await check();
        if (u) {
          let skipped: string | null = null;
          try {
            skipped = localStorage.getItem(SKIP_KEY);
          } catch {
            /* storage indisponível */
          }
          if (manual || skipped !== u.version) setUpdate(u);
        } else if (manual) {
          notify("success", "Você já está na versão mais recente.");
        }
      } catch (e) {
        // Em desenvolvimento (sem chave/endpoint configurados) falhar é esperado
        console.warn("verificação de atualização falhou:", e);
        if (manual) notify("error", `Não foi possível verificar atualizações: ${errorMessage(e)}`);
      } finally {
        setChecking(false);
      }
    },
    [notify],
  );

  useEffect(() => {
    const t = setTimeout(() => void checkNow(false), 2500);
    return () => clearTimeout(t);
  }, [checkNow]);

  return { update, dismiss: () => setUpdate(null), checkNow, checking };
}

export function UpdateDialog({ update, onClose }: { update: Update | null; onClose: () => void }) {
  const [phase, setPhase] = useState<Phase>("available");
  const [downloaded, setDownloaded] = useState(0);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (update) {
      setPhase("available");
      setDownloaded(0);
      setTotal(0);
      setError(null);
    }
  }, [update]);

  if (!update) return null;

  const install = async () => {
    setPhase("downloading");
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") setTotal(event.data.contentLength ?? 0);
        else if (event.event === "Progress") setDownloaded((d) => d + event.data.chunkLength);
        else if (event.event === "Finished") setPhase("installing");
      });
      await relaunch();
    } catch (e) {
      setError(errorMessage(e));
      setPhase("error");
    }
  };

  const later = () => {
    try {
      localStorage.setItem(SKIP_KEY, update.version);
    } catch {
      /* ignora */
    }
    onClose();
  };

  const pct = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0;
  const busy = phase === "downloading" || phase === "installing";

  return (
    <Modal open title="Atualização disponível" onClose={busy ? () => undefined : onClose}>
      <div className="space-y-4">
        <div className="flex items-center gap-3">
          <div className="grid h-11 w-11 place-items-center rounded-xl bg-accent/20 text-accent">
            <Rocket size={20} />
          </div>
          <div>
            <p className="font-semibold">Aurora Launcher {update.version}</p>
            <p className="text-xs text-muted">
              Você está na {update.currentVersion}
              {update.date && ` · publicada em ${update.date.slice(0, 10)}`}
            </p>
          </div>
        </div>

        {update.body && (
          <div className="selectable max-h-48 overflow-y-auto whitespace-pre-wrap rounded-lg bg-bg p-3 text-sm text-zinc-300">
            {update.body}
          </div>
        )}

        {busy && (
          <div className="space-y-1.5">
            <div className="h-2 overflow-hidden rounded-full bg-panel-3">
              <div className="h-full bg-gradient-to-r from-accent to-fuchsia-500 transition-[width]" style={{ width: `${phase === "installing" ? 100 : pct}%` }} />
            </div>
            <p className="font-mono text-[11px] text-muted">
              {phase === "installing"
                ? "Instalando… o launcher vai reiniciar sozinho."
                : `${formatBytes(downloaded)}${total ? ` / ${formatBytes(total)}` : ""}`}
            </p>
          </div>
        )}

        {error && <p className="selectable rounded-md bg-err/10 px-3 py-2 text-sm text-err">{error}</p>}

        <div className="flex justify-end gap-2">
          {!busy && (
            <Button variant="ghost" onClick={later}>
              Depois
            </Button>
          )}
          <Button variant="primary" disabled={busy} onClick={() => void install()}>
            {busy ? <Spinner size={14} /> : <Download size={15} />}
            {phase === "error" ? "Tentar de novo" : busy ? "Atualizando…" : "Atualizar agora"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
