import { useEffect, useRef, useState } from "react";
import { Check, Download, Package, Search } from "lucide-react";
import { api, errorMessage, formatNumber } from "../lib/api";
import type { ContentKind, InstanceConfig, SearchHit, SortIndex } from "../lib/types";
import { KindSwitch } from "./ModsTab";
import { Button, Spinner, cx } from "./ui";

const SORTS: { id: SortIndex; label: string }[] = [
  { id: "relevance", label: "Relevância" },
  { id: "downloads", label: "Downloads" },
  { id: "follows", label: "Seguidores" },
  { id: "updated", label: "Atualizados" },
  { id: "newest", label: "Novos" },
];

interface Props {
  instance: InstanceConfig;
  locked: boolean;
  notify: (kind: "error" | "success" | "info", text: string) => void;
  onInstalled: () => void;
}

export function BrowseTab({ instance, locked, notify, onInstalled }: Props) {
  const allowMods = instance.loader !== "vanilla";
  const [kind, setKind] = useState<ContentKind>(allowMods ? "mod" : "resourcepack");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortIndex>("relevance");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const reqId = useRef(0);

  // Projetos já instalados (para marcar "Instalado")
  useEffect(() => {
    api
      .listContent(instance.id, kind)
      .then((list) => setInstalled(new Set(list.map((i) => i.projectId).filter((x): x is string => !!x))))
      .catch(() => undefined);
  }, [instance.id, kind]);

  // Busca com debounce; ignora respostas antigas fora de ordem
  useEffect(() => {
    const id = ++reqId.current;
    const t = setTimeout(async () => {
      setLoading(true);
      try {
        const r = await api.searchContent(instance.id, kind, query, sort, 0);
        if (id !== reqId.current) return;
        setHits(r.hits);
        setTotal(r.totalHits);
      } catch (e) {
        if (id === reqId.current) notify("error", errorMessage(e));
      } finally {
        if (id === reqId.current) setLoading(false);
      }
    }, 300);
    return () => clearTimeout(t);
  }, [instance.id, kind, query, sort, notify]);

  const loadMore = async () => {
    setLoading(true);
    try {
      const r = await api.searchContent(instance.id, kind, query, sort, hits.length);
      setHits((h) => [...h, ...r.hits]);
    } catch (e) {
      notify("error", errorMessage(e));
    } finally {
      setLoading(false);
    }
  };

  const install = async (hit: SearchHit) => {
    setInstalling(hit.projectId);
    try {
      const report = await api.installContent(instance.id, kind, [hit.projectId]);
      if (report.skipped.length) notify("error", `${hit.title} não tem versão para ${instance.minecraftVersion}.`);
      else {
        notify("success", `Instalado: ${report.installed.join(", ")}`);
        setInstalled((s) => new Set(s).add(hit.projectId));
        onInstalled();
      }
    } catch (e) {
      notify("error", errorMessage(e));
    } finally {
      setInstalling(null);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <KindSwitch kind={kind} onChange={setKind} allowMods={allowMods} />
        <div className="relative min-w-60 flex-1">
          <Search size={15} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Buscar no Modrinth para ${instance.minecraftVersion}…`}
            className="w-full rounded-md border border-line bg-bg py-2 pl-9 pr-3 text-sm outline-none focus:border-accent"
          />
        </div>
        <select
          value={sort}
          onChange={(e) => setSort(e.target.value as SortIndex)}
          className="rounded-md border border-line bg-bg px-2 py-2 text-sm outline-none"
        >
          {SORTS.map((s) => (
            <option key={s.id} value={s.id}>
              {s.label}
            </option>
          ))}
        </select>
      </div>
      <p className="text-xs text-muted">
        {loading ? "Buscando…" : `${formatNumber(total)} resultados compatíveis com ${instance.loader} ${instance.minecraftVersion}`}
      </p>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="grid grid-cols-1 gap-2 xl:grid-cols-2">
          {hits.map((hit) => {
            const isInstalled = installed.has(hit.projectId);
            return (
              <div key={hit.projectId} className="flex gap-3 rounded-xl border border-line bg-panel p-3 transition hover:border-panel-3">
                {hit.iconUrl ? (
                  <img src={hit.iconUrl} className="h-12 w-12 shrink-0 rounded-lg bg-panel-3 object-cover" loading="lazy" />
                ) : (
                  <div className="grid h-12 w-12 shrink-0 place-items-center rounded-lg bg-panel-3 text-muted">
                    <Package size={18} />
                  </div>
                )}
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold">
                    {hit.title} <span className="font-normal text-muted">por {hit.author}</span>
                  </p>
                  <p className="line-clamp-2 text-xs text-muted">{hit.description}</p>
                  <p className="mt-1 flex items-center gap-1 text-[11px] text-muted">
                    <Download size={11} /> {formatNumber(hit.downloads)}
                  </p>
                </div>
                <Button
                  variant={isInstalled ? "subtle" : "primary"}
                  className="self-center"
                  disabled={isInstalled || locked || installing !== null}
                  onClick={() => void install(hit)}
                >
                  {installing === hit.projectId ? <Spinner size={14} /> : isInstalled ? <Check size={15} /> : <Download size={15} />}
                  {isInstalled ? "Instalado" : "Instalar"}
                </Button>
              </div>
            );
          })}
        </div>
        {hits.length < total && hits.length > 0 && (
          <div className="py-4 text-center">
            <Button variant="subtle" disabled={loading} onClick={() => void loadMore()}>
              {loading && <Spinner size={14} />} Carregar mais
            </Button>
          </div>
        )}
        {!loading && hits.length === 0 && (
          <p className={cx("py-10 text-center text-sm text-muted")}>Nenhum resultado para esta versão.</p>
        )}
      </div>
    </div>
  );
}
