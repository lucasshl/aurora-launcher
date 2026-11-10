import { useEffect, useMemo, useState } from "react";
import { FileCode2, RotateCcw, Save, Search } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import type { ConfigFile, InstanceConfig } from "../lib/types";
import { Button, Spinner, cx } from "./ui";

interface Props {
  instance: InstanceConfig;
  notify: (kind: "error" | "success" | "info", text: string) => void;
}

export function ConfigsTab({ instance, notify }: Props) {
  const [files, setFiles] = useState<ConfigFile[]>([]);
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [original, setOriginal] = useState("");
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const dirty = draft !== original;

  useEffect(() => {
    api.listConfigs(instance.id).then(setFiles).catch((e) => notify("error", errorMessage(e)));
    setSelected(null);
  }, [instance.id, notify]);

  const open = async (path: string) => {
    if (dirty && !window.confirm("Descartar alterações não salvas?")) return;
    try {
      const text = await api.readConfig(instance.id, path);
      setSelected(path);
      setOriginal(text);
      setDraft(text);
      setError(null);
    } catch (e) {
      notify("error", errorMessage(e));
    }
  };

  const save = async () => {
    if (!selected) return;
    setSaving(true);
    setError(null);
    try {
      await api.writeConfig(instance.id, selected, draft);
      setOriginal(draft);
      notify("success", `${selected} salvo (backup em .bak).`);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  // Agrupa por pasta de primeiro nível (normalmente = mod)
  const groups = useMemo(() => {
    const q = filter.toLowerCase();
    const map = new Map<string, ConfigFile[]>();
    for (const f of files.filter((f) => !q || f.path.toLowerCase().includes(q))) {
      const parts = f.path.split("/");
      const group = parts.length > 2 ? parts[1]! : parts[0] === "config" ? "config" : "jogo";
      map.set(group, [...(map.get(group) ?? []), f]);
    }
    return [...map.entries()];
  }, [files, filter]);

  return (
    <div className="grid h-full min-h-0 grid-cols-[18rem_1fr] gap-3">
      <div className="flex min-h-0 flex-col overflow-hidden rounded-xl border border-line bg-panel">
        <div className="relative border-b border-line p-2">
          <Search size={13} className="absolute left-4 top-1/2 -translate-y-1/2 text-muted" />
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filtrar arquivos"
            className="w-full rounded bg-bg py-1.5 pl-7 pr-2 text-xs outline-none focus:ring-1 focus:ring-accent"
          />
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-1.5">
          {groups.length === 0 && (
            <p className="p-4 text-center text-xs text-muted">Nenhuma config ainda. Abra o jogo uma vez para os mods gerarem.</p>
          )}
          {groups.map(([group, list]) => (
            <div key={group} className="mb-2">
              <p className="px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-muted">{group}</p>
              {list.map((f) => (
                <button
                  key={f.path}
                  onClick={() => void open(f.path)}
                  className={cx(
                    "flex w-full items-center gap-2 truncate rounded px-2 py-1.5 text-left text-xs transition",
                    selected === f.path ? "bg-accent/20 text-white" : "text-zinc-400 hover:bg-panel-3",
                  )}
                >
                  <FileCode2 size={13} className="shrink-0" />
                  <span className="truncate">{f.path.split("/").pop()}</span>
                </button>
              ))}
            </div>
          ))}
        </div>
      </div>

      <div className="flex min-h-0 flex-col overflow-hidden rounded-xl border border-line bg-[#0a0b0e]">
        {selected ? (
          <>
            <div className="flex items-center gap-2 border-b border-line px-3 py-2">
              <span className="truncate font-mono text-xs">{selected}</span>
              {dirty && <span className="rounded bg-warn/15 px-1.5 text-[10px] font-bold text-warn">não salvo</span>}
              <div className="ml-auto flex gap-1">
                <Button variant="ghost" disabled={!dirty} onClick={() => setDraft(original)}>
                  <RotateCcw size={14} /> Desfazer
                </Button>
                <Button variant="primary" disabled={!dirty || saving} onClick={() => void save()}>
                  {saving ? <Spinner size={14} /> : <Save size={14} />} Salvar
                </Button>
              </div>
            </div>
            {error && <p className="selectable border-b border-err/30 bg-err/10 px-3 py-2 text-xs text-err">{error}</p>}
            <textarea
              value={draft}
              spellCheck={false}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if ((e.ctrlKey || e.metaKey) && e.key === "s") {
                  e.preventDefault();
                  void save();
                }
                if (e.key === "Tab") {
                  e.preventDefault();
                  const el = e.currentTarget;
                  const { selectionStart: s, selectionEnd: end } = el;
                  setDraft(draft.slice(0, s) + "  " + draft.slice(end));
                  requestAnimationFrame(() => el.setSelectionRange(s + 2, s + 2));
                }
              }}
              className="min-h-0 flex-1 resize-none bg-transparent p-3 font-mono text-[12px] leading-relaxed text-zinc-200 outline-none"
            />
          </>
        ) : (
          <div className="grid h-full place-items-center text-sm text-muted">Escolha um arquivo para editar (Ctrl+S salva).</div>
        )}
      </div>
    </div>
  );
}
