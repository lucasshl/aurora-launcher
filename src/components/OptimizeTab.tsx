import { useCallback, useEffect, useState } from "react";
import { Check, Gauge, Rocket, Sparkles, Swords, Zap } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import type { CuratedCategory, CuratedStatus, GraphicsPreset, InstanceConfig } from "../lib/types";
import { Button, Spinner, Toggle, cx } from "./ui";

const CATEGORY_LABEL: Record<CuratedCategory, string> = {
  render: "FPS",
  logic: "Lógica",
  memory: "RAM",
  worldgen: "Mundo",
  startup: "Abertura",
  network: "Rede",
  pvp: "PvP",
  hud: "HUD",
  shaders: "Shaders",
  utility: "Útil",
};

const PRESETS: { id: GraphicsPreset; title: string; desc: string; icon: typeof Gauge }[] = [
  { id: "potato", title: "PC fraco", desc: "Distância 5, gráficos rápidos, sem nuvens, sombras ou partículas. Mira em 50+ FPS.", icon: Zap },
  { id: "balanced", title: "Equilibrado", desc: "Distância 8, nuvens rápidas, suavização leve.", icon: Gauge },
  { id: "quality", title: "Qualidade", desc: "Distância 12, gráficos bonitos, para PCs bons.", icon: Sparkles },
];

interface Props {
  instance: InstanceConfig;
  locked: boolean;
  totalMemoryMb: number;
  notify: (kind: "error" | "success" | "info", text: string) => void;
  onChange: (inst: InstanceConfig) => void;
  onInstalled: () => void;
  onConvertToFabric: () => Promise<void>;
}

export function OptimizeTab({ instance, locked, totalMemoryMb, notify, onChange, onInstalled, onConvertToFabric }: Props) {
  const modded = instance.loader !== "vanilla";
  const [converting, setConverting] = useState(false);

  return (
    <div className="h-full min-h-0 space-y-4 overflow-y-auto pr-1">
      {!modded && (
        <div className="flex items-center gap-4 rounded-xl border border-accent/40 bg-accent/10 p-4">
          <Zap size={22} className="shrink-0 text-accent" />
          <div className="flex-1 text-sm">
            <p className="font-semibold">Otimizar este perfil com um clique</p>
            <p className="text-xs text-muted">
              Vanilla não carrega mods de desempenho. Este botão troca o perfil para <b>Fabric</b> (mesma versão, mesmos mundos),
              instala Sodium, Lithium, C2ME, FerriteCore e outros, e aplica o preset para PC fraco.
            </p>
          </div>
          <Button
            variant="primary"
            disabled={locked || converting}
            onClick={async () => {
              setConverting(true);
              try {
                await onConvertToFabric();
              } finally {
                setConverting(false);
              }
            }}
          >
            {converting ? <Spinner size={14} /> : <Rocket size={15} />} Otimizar agora
          </Button>
        </div>
      )}

      <section className="rounded-xl border border-line bg-panel p-4">
        <h3 className="mb-1 flex items-center gap-2 text-sm font-semibold">
          <Gauge size={16} className="text-accent" /> Preset gráfico
        </h3>
        <p className="mb-3 text-xs text-muted">Grava no options.txt (e nas opções do Sodium, se instalado). Pode ajustar depois no jogo.</p>
        <div className="grid grid-cols-3 gap-2">
          {PRESETS.map((p) => (
            <button
              key={p.id}
              disabled={locked}
              onClick={async () => {
                try {
                  await api.applyGraphicsPreset(instance.id, p.id);
                  notify("success", `Preset "${p.title}" aplicado.`);
                } catch (e) {
                  notify("error", errorMessage(e));
                }
              }}
              className="rounded-lg border border-line bg-bg p-3 text-left transition hover:border-accent disabled:opacity-50"
            >
              <p className="flex items-center gap-2 text-sm font-semibold">
                <p.icon size={15} /> {p.title}
              </p>
              <p className="mt-1 text-xs text-muted">{p.desc}</p>
            </button>
          ))}
        </div>
      </section>

      <section className="rounded-xl border border-line bg-panel p-4">
        <h3 className="mb-2 flex items-center gap-2 text-sm font-semibold">
          <Rocket size={16} className="text-accent" /> Inicialização
        </h3>
        <Toggle
          checked={instance.experimentalCds}
          onChange={(v) => onChange({ ...instance, experimentalCds: v })}
          label={
            <span>
              AppCDS (experimental)
              <span className="block text-xs text-warn">
                Pode impedir o jogo de abrir em alguns PCs (fecha sem erro). Deixe desligado se der problema.
              </span>
            </span>
          }
        />
        {totalMemoryMb > 0 && totalMemoryMb <= 6144 && (
          <p className="mt-2 rounded-md bg-bg px-3 py-2 text-xs text-muted">
            Seu PC tem {(totalMemoryMb / 1024).toFixed(0)} GB de RAM: use 2–3 GB no slider. RAM demais deixa o Windows sem memória
            e causa travadas.
          </p>
        )}
      </section>

      {modded && (
        <>
          <CuratedSection
            title="Pacote de desempenho"
            icon={Zap}
            pack="performance"
            instance={instance}
            locked={locked}
            notify={notify}
            onInstalled={onInstalled}
            footer="Depois de instalar o Chunky, use /chunky radius 1000 e /chunky start num mundo para pré-gerar o mapa."
          />
          <CuratedSection
            title="Pacote PvP"
            icon={Swords}
            pack="pvp"
            instance={instance}
            locked={locked}
            notify={notify}
            onInstalled={onInstalled}
            footer="Freelook: segure Alt. Zoom: C. Atalhos ficam em Opções → Controles."
          />
        </>
      )}
    </div>
  );
}

function CuratedSection({
  title,
  icon: Icon,
  pack,
  instance,
  locked,
  notify,
  onInstalled,
  footer,
}: {
  title: string;
  icon: typeof Zap;
  pack: "performance" | "pvp";
  instance: InstanceConfig;
  locked: boolean;
  notify: Props["notify"];
  onInstalled: () => void;
  footer: string;
}) {
  const [items, setItems] = useState<CuratedStatus[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setItems(null);
    try {
      const list = await api.curatedMods(instance.id, pack);
      setItems(list);
      setSelected(new Set(list.filter((i) => i.recommended && i.availableSlug && !i.installed).map((i) => i.key)));
    } catch (e) {
      notify("error", errorMessage(e));
      setItems([]);
    }
  }, [instance.id, pack, notify]);

  useEffect(() => {
    void load();
  }, [load]);

  const install = async () => {
    setBusy(true);
    try {
      const r = await api.installCurated(instance.id, [...selected]);
      notify("success", `Instalados ${r.installed.length} arquivos (inclui dependências).`);
      if (r.skipped.length) notify("info", `Sem versão compatível: ${r.skipped.join(", ")}`);
      onInstalled();
      await load();
    } catch (e) {
      notify("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="rounded-xl border border-line bg-panel p-4">
      <div className="mb-3 flex items-center gap-2">
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          <Icon size={16} className="text-accent" /> {title}
        </h3>
        <span className="text-xs text-muted">
          para {instance.loader} {instance.minecraftVersion}
        </span>
        <Button variant="primary" className="ml-auto" disabled={busy || locked || selected.size === 0} onClick={() => void install()}>
          {busy && <Spinner size={14} />} Instalar selecionados ({selected.size})
        </Button>
      </div>
      {items === null ? (
        <div className="flex items-center gap-2 py-4 text-sm text-muted">
          <Spinner size={14} /> Verificando compatibilidade no Modrinth…
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-1.5 lg:grid-cols-2">
          {items.map((m) => {
            const available = !!m.availableSlug;
            const checked = selected.has(m.key);
            return (
              <button
                key={m.key}
                disabled={!available || m.installed}
                onClick={() =>
                  setSelected((s) => {
                    const n = new Set(s);
                    if (n.has(m.key)) n.delete(m.key);
                    else n.add(m.key);
                    return n;
                  })
                }
                className={cx(
                  "flex items-start gap-3 rounded-lg border p-2.5 text-left transition",
                  checked ? "border-accent bg-accent/10" : "border-line bg-bg hover:border-panel-3",
                  (!available || m.installed) && "cursor-default opacity-60",
                )}
              >
                <span
                  className={cx(
                    "mt-0.5 grid h-4 w-4 shrink-0 place-items-center rounded border",
                    m.installed ? "border-ok bg-ok" : checked ? "border-accent bg-accent" : "border-line",
                  )}
                >
                  {(checked || m.installed) && <Check size={11} strokeWidth={3} />}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex items-center gap-2 text-sm font-medium">
                    {m.name}
                    <span className="rounded bg-panel-3 px-1.5 py-px text-[10px] font-semibold uppercase text-muted">
                      {CATEGORY_LABEL[m.category]}
                    </span>
                  </span>
                  <span className="block text-xs text-muted">{m.description}</span>
                  <span className={cx("block text-[11px]", m.installed ? "text-ok" : available ? "text-muted" : "text-warn")}>
                    {m.installed ? "Instalado" : available ? `v${m.version}` : "Sem versão para esta instância"}
                  </span>
                </span>
              </button>
            );
          })}
        </div>
      )}
      <p className="mt-3 text-xs text-muted">{footer}</p>
    </section>
  );
}
