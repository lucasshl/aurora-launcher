import { useCallback, useEffect, useRef, useState } from "react";
import { events } from "../lib/api";
import type { ExitInfo, LogLine, ProgressEvent } from "../lib/types";

const MAX_LINES = 5000;


/**
 * Assina os eventos do core e mantém, por instância: progresso atual,
 * velocidade de download, buffer circular de logs e último status de saída.
 */
export function useLaunchEvents() {
  const [progress, setProgress] = useState<Record<string, ProgressEvent | undefined>>({});
  const [speed, setSpeed] = useState<Record<string, number>>({});
  const [logs, setLogs] = useState<Record<string, LogLine[]>>({});
  const [exits, setExits] = useState<Record<string, ExitInfo | undefined>>({});
  /** Horário (ms) do último evento de saída por instância — lido de forma síncrona. */
  const exitAt = useRef<Record<string, number>>({});
  const lastSample = useRef<Record<string, { t: number; bytes: number; stage: string }>>({});

  useEffect(() => {
    const unsubs = Promise.all([
      events.onProgress((p) => {
        setProgress((prev) => ({ ...prev, [p.instanceId]: p }));

        // Velocidade com média móvel exponencial, reiniciada a cada etapa
        const now = performance.now();
        const prev = lastSample.current[p.instanceId];
        if (prev && prev.stage === p.stage && now - prev.t >= 250) {
          const inst = ((p.bytesDone - prev.bytes) / (now - prev.t)) * 1000;
          setSpeed((s) => ({ ...s, [p.instanceId]: Math.max(0, (s[p.instanceId] ?? inst) * 0.6 + inst * 0.4) }));
          lastSample.current[p.instanceId] = { t: now, bytes: p.bytesDone, stage: p.stage };
        } else if (!prev || prev.stage !== p.stage) {
          lastSample.current[p.instanceId] = { t: now, bytes: p.bytesDone, stage: p.stage };
          setSpeed((s) => ({ ...s, [p.instanceId]: 0 }));
        }
      }),
      events.onLogs((batch) => {
        const grouped: Record<string, LogLine[]> = {};
        for (const line of batch) (grouped[line.instanceId] ??= []).push(line);
        setLogs((prev) => {
          const next = { ...prev };
          for (const [id, lines] of Object.entries(grouped)) {
            const merged = [...(prev[id] ?? []), ...lines];
            next[id] = merged.length > MAX_LINES ? merged.slice(-MAX_LINES) : merged;
          }
          return next;
        });
      }),
      events.onExit((info) => {
        exitAt.current[info.instanceId] = Date.now();
        setExits((prev) => ({ ...prev, [info.instanceId]: info }));
      }),
    ]);
    return () => {
      void unsubs.then((fns) => fns.forEach((f) => f()));
    };
  }, []);

  const clearProgress = useCallback((id: string) => {
    setProgress((p) => ({ ...p, [id]: undefined }));
    delete lastSample.current[id];
  }, []);

  const clearLogs = useCallback((id: string) => setLogs((l) => ({ ...l, [id]: [] })), []);
  const clearExit = useCallback((id: string) => setExits((e) => ({ ...e, [id]: undefined })), []);

  /** Linha local (ex.: erro antes de o core emitir eventos). */
  const pushLocalLog = useCallback((line: LogLine) => {
    setLogs((l) => ({ ...l, [line.instanceId]: [...(l[line.instanceId] ?? []), line] }));
  }, []);

  const exitedSince = useCallback((id: string, since: number) => (exitAt.current[id] ?? 0) >= since, []);

  return { progress, speed, logs, exits, clearProgress, clearLogs, clearExit, pushLocalLog, exitedSince };
}
