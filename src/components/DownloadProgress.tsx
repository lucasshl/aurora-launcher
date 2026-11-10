import { formatBytes } from "../lib/api";
import type { ProgressEvent, Stage } from "../lib/types";
import { cx } from "./ui";

const STAGE_LABEL: Record<Stage, string> = {
  manifest: "Consultando versões",
  loader: "Preparando modloader",
  java: "Verificando Java",
  client: "Baixando client",
  libraries: "Baixando bibliotecas",
  asset_index: "Baixando índice de assets",
  assets: "Baixando assets",
  mods: "Instalando CustomSkinLoader",
  natives: "Extraindo nativos",
  launching: "Iniciando a JVM",
};

const STAGE_ORDER: Stage[] = ["manifest", "loader", "java", "client", "libraries", "asset_index", "assets", "mods", "natives", "launching"];

export function DownloadProgress({ progress, speed }: { progress: ProgressEvent; speed: number }) {
  const { stage, filesDone, filesTotal, bytesDone, bytesTotal, current } = progress;
  const byBytes = bytesTotal > 0;
  const ratio = byBytes ? bytesDone / bytesTotal : filesTotal > 0 ? filesDone / filesTotal : 0;
  const indeterminate = !byBytes && filesTotal === 0;
  const pct = Math.min(100, Math.round(ratio * 100));
  const step = STAGE_ORDER.indexOf(stage) + 1;

  return (
    <div className="animate-fade-in space-y-2">
      <div className="flex items-baseline justify-between gap-4 text-sm">
        <span className="font-medium">
          {STAGE_LABEL[stage]}
          <span className="ml-2 text-xs text-muted">
            etapa {step}/{STAGE_ORDER.length}
          </span>
        </span>
        {!indeterminate && <span className="font-mono text-xs tabular-nums text-muted">{pct}%</span>}
      </div>

      <div className="relative h-2 overflow-hidden rounded-full bg-panel-3">
        {indeterminate ? (
          <div className="absolute inset-0 animate-shimmer bg-[linear-gradient(90deg,transparent,var(--color-accent),transparent)] bg-[length:200%_100%]" />
        ) : (
          <div
            className={cx("h-full rounded-full bg-gradient-to-r from-accent to-fuchsia-500 transition-[width] duration-200")}
            style={{ width: `${pct}%` }}
          />
        )}
      </div>

      <div className="flex justify-between gap-4 font-mono text-[11px] tabular-nums text-muted">
        <span className="truncate">
          {filesTotal > 0 && `${filesDone}/${filesTotal} arquivos`}
          {current && ` · ${current}`}
        </span>
        <span className="shrink-0">
          {byBytes && `${formatBytes(bytesDone)} / ${formatBytes(bytesTotal)}`}
          {speed > 1024 && ` · ${formatBytes(speed)}/s`}
        </span>
      </div>
    </div>
  );
}
