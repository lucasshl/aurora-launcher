import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Box,
  Check,
  Cloud,
  Download,
  ExternalLink,
  Feather,
  Image as ImageIcon,
  Pause,
  Pencil,
  Play,
  Plus,
  RotateCw,
  Search,
  Share2,
  Shirt,
  Sparkles,
  Trash2,
  Upload,
  Users,
  X,
} from "lucide-react";
import { IdleAnimation, SkinViewer, WalkingAnimation } from "skinview3d";
import { api, errorMessage, formatNumber } from "../lib/api";
import { COSMETICS_3D, Cosmetics3DController, SLOTS, SLOT_LABEL, toggleItem } from "../lib/cosmetics3d";
import {
  ACCESSORIES,
  CAPES,
  DEFAULT_CHARACTER,
  clearHeadOverlay,
  dataUrlToImage,
  generateCharacter,
  imageToDataUrl,
  renderCape,
  type Accessory,
} from "../lib/skin";
import type { Account, Cosmetic, CosmeticKind, CosmeticsModStatus, GalleryItem, MojangCape, Settings, SkinModel, Wardrobe } from "../lib/types";
import { SkinHead } from "./SkinHead";
import { Button, Spinner, cx } from "./ui";

interface Props {
  account: Account | null;
  settings: Settings | null;
  wardrobe: Wardrobe;
  textures: Record<string, string>;
  refresh: (w?: Wardrobe) => Promise<void>;
  onEdit: (c: Cosmetic | null) => void;
  notify: (kind: "error" | "success" | "info", text: string) => void;
}

type Tab = "items3d" | "mine" | "catalog" | "community" | "ears";

const STEVE_FALLBACK = imageToDataUrlSafe();
function imageToDataUrlSafe(): string {
  try {
    return imageToDataUrl(generateCharacter(DEFAULT_CHARACTER));
  } catch {
    return "";
  }
}

export function WardrobePage({ account, settings, wardrobe, textures, refresh, onEdit, notify }: Props) {
  const [tab, setTab] = useState<Tab>("items3d");
  const [preview, setPreview] = useState<{ skin?: { tex: string; model: SkinModel }; cape?: string | null }>({});
  const [mojangCapes, setMojangCapes] = useState<MojangCape[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const equipped = account ? wardrobe.equipped[account.id] : undefined;
  const equippedSkin = wardrobe.items.find((c) => c.id === equipped?.skin);
  const equippedCape = wardrobe.items.find((c) => c.id === equipped?.cape);
  const shownSkin = preview.skin ?? (equippedSkin ? { tex: textures[equippedSkin.id] ?? "", model: equippedSkin.model } : undefined);
  const shownCape = preview.cape === undefined ? (equippedCape ? textures[equippedCape.id] : undefined) : preview.cape ?? undefined;
  const hasServer = !!settings?.cosmeticsServer;
  const equipped3d = equipped?.items3d ?? [];
  const [preview3d, setPreview3d] = useState<string[] | null>(null);
  const shown3d = preview3d ?? equipped3d;

  const toggle3d = (id: string) => {
    if (!account) {
      setPreview3d((p) => toggleItem(p ?? [], id));
      return;
    }
    void run(`3d-${id}`, async () => {
      await refresh(await api.setItems3d(account.id, toggleItem(equipped3d, id)));
      setPreview3d(null);
    });
  };

  useEffect(() => {
    setMojangCapes(null);
    if (account?.kind === "microsoft") api.mojangCapes(account.id).then(setMojangCapes).catch(() => setMojangCapes([]));
  }, [account]);

  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    try {
      await fn();
    } catch (e) {
      notify("error", errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const equip = (kind: CosmeticKind, id: string | null) =>
    account &&
    run(`equip-${kind}`, async () => {
      await refresh(await api.equipCosmetic(account.id, kind, id));
      setPreview({});
    });

  const importFile = (kind: CosmeticKind) =>
    run(`import-${kind}`, async () => {
      const file = await openDialog({ multiple: false, filters: [{ name: "PNG", extensions: ["png"] }] });
      if (typeof file !== "string") return;
      const name = file.split(/[\\/]/).pop()?.replace(/\.png$/i, "") ?? "";
      await refresh(await api.importCosmetic(file, kind, name, null));
      notify("success", kind === "skin" ? "Skin adicionada." : "Capa adicionada.");
    });

  /** Aplica um acessório sobre a skin equipada (ou um personagem padrão) e salva como nova skin. */
  const addAccessory = (acc: Accessory) =>
    run(`acc-${acc.id}`, async () => {
      const baseTex = equippedSkin ? textures[equippedSkin.id] : undefined;
      const img = baseTex ? await dataUrlToImage(baseTex) : generateCharacter(DEFAULT_CHARACTER);
      clearHeadOverlay(img);
      acc.draw(img);
      const baseName = equippedSkin?.name ?? "Personagem";
      const c = await api.importCosmeticData(imageToDataUrl(img), "skin", `${baseName} + ${acc.name}`, equippedSkin?.model ?? "classic");
      if (account) await refresh(await api.equipCosmetic(account.id, "skin", c.id));
      else await refresh();
      notify("success", `${acc.name} aplicado${account ? " e equipado" : ""}.`);
    });

  const addCape = (id: string) =>
    run(`cape-${id}`, async () => {
      const design = CAPES.find((c) => c.id === id)!;
      const c = await api.importCosmeticData(imageToDataUrl(renderCape(design)), "cape", `Capa ${design.name}`, null);
      if (account) await refresh(await api.equipCosmetic(account.id, "cape", c.id));
      else await refresh();
      notify("success", `Capa ${design.name} ${account ? "equipada" : "adicionada"}.`);
    });

  const TABS: { id: Tab; label: string; icon: typeof Shirt }[] = [
    { id: "items3d", label: "Itens 3D", icon: Box },
    { id: "mine", label: "Meu guarda-roupa", icon: Shirt },
    { id: "catalog", label: "Catálogo Aurora", icon: Sparkles },
    { id: "community", label: "Comunidade", icon: Users },
    { id: "ears", label: "Asas, orelhas e rabo", icon: Feather },
  ];

  return (
    <div className="grid h-full min-h-0 grid-cols-[21rem_1fr] gap-5 p-6">
      {/* Preview 3D */}
      <section className="flex min-h-0 flex-col rounded-2xl border border-line bg-gradient-to-b from-panel-2 to-bg p-4">
        <h2 className="mb-1 flex items-center gap-2 text-lg font-bold">
          <Shirt size={18} /> Guarda-roupa
        </h2>
        <p className="mb-3 text-xs text-muted">
          {account ? (
            <>
              Equipando em <b className="text-zinc-200">{account.username}</b> ({account.kind === "microsoft" ? "original" : "pirata"})
            </>
          ) : (
            "Adicione uma conta para equipar cosméticos."
          )}
        </p>
        <SkinPreview skin={shownSkin?.tex || STEVE_FALLBACK} model={shownSkin?.model ?? "classic"} cape={shownCape} items3d={shown3d} />
        {(preview.skin || preview.cape !== undefined) && (
          <p className="mt-2 text-center text-xs text-warn">Pré-visualização. Clique em "Equipar" para usar.</p>
        )}

        <div className="mt-auto space-y-2 pt-4">
          <Button variant="primary" className="w-full" onClick={() => onEdit(equippedSkin ?? null)}>
            <Pencil size={15} /> {equippedSkin ? "Editar personagem atual" : "Criar personagem"}
          </Button>
          {account?.kind === "microsoft" && (
            <Button
              variant="subtle"
              className="w-full"
              disabled={!equippedSkin || busy !== null}
              onClick={() =>
                void run("mojang", async () => {
                  await api.uploadSkinToMojang(account.id, equippedSkin!.id);
                  notify("success", "Skin enviada para a sua conta Microsoft.");
                })
              }
            >
              {busy === "mojang" ? <Spinner size={14} /> : <Upload size={15} />} Enviar skin para a Mojang
            </Button>
          )}
          {account && (
            <Button
              variant="subtle"
              className="w-full"
              disabled={!hasServer || busy !== null}
              title={hasServer ? undefined : "Configure o servidor de cosméticos em Configurações"}
              onClick={() =>
                void run("sync", async () => {
                  await api.syncCosmetics(account.id);
                  notify("success", "Skin e capa publicadas: outros jogadores do launcher já podem ver.");
                })
              }
            >
              {busy === "sync" ? <Spinner size={14} /> : <Cloud size={15} />} Mostrar para outros jogadores
            </Button>
          )}
        </div>
      </section>

      <section className="flex min-h-0 flex-col gap-4">
        <nav className="flex gap-1 border-b border-line">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={cx(
                "-mb-px flex items-center gap-2 border-b-2 px-3.5 py-2 text-sm font-medium transition",
                tab === t.id ? "border-accent text-white" : "border-transparent text-muted hover:text-zinc-200",
              )}
            >
              <t.icon size={15} /> {t.label}
            </button>
          ))}
        </nav>

        <div className="min-h-0 flex-1 space-y-5 overflow-y-auto pr-1">
          {tab === "items3d" && (
            <Items3DPanel
              equipped={shown3d}
              canEquip={!!account}
              busy={busy}
              onToggle={toggle3d}
              onClear={() =>
                account ? void run("3d-clear", async () => refresh(await api.setItems3d(account.id, []))) : setPreview3d([])
              }
            />
          )}

          {tab === "mine" && (
            <>
              <Gallery
                title="Skins"
                kind="skin"
                items={wardrobe.items.filter((c) => c.kind === "skin")}
                textures={textures}
                equippedId={equipped?.skin ?? null}
                canEquip={!!account}
                busy={busy}
                onImport={() => void importFile("skin")}
                onCreate={() => onEdit(null)}
                onPreview={(c) => setPreview((p) => ({ ...p, skin: { tex: textures[c.id] ?? "", model: c.model } }))}
                onEquip={(id) => void equip("skin", id)}
                onEdit={(c) => onEdit(c)}
                onShare={
                  hasServer && account
                    ? (c) =>
                        void run("share", async () => {
                          await api.galleryPublish(account.id, c.id, c.name);
                          notify("success", `"${c.name}" compartilhada na comunidade.`);
                        })
                    : undefined
                }
                onRemove={(id) => void run("remove", async () => refresh(await api.removeCosmetic(id)))}
                onModel={(id, m) => void run("model", async () => refresh(await api.setCosmeticModel(id, m)))}
              />
              <Gallery
                title="Capas e élitras"
                kind="cape"
                items={wardrobe.items.filter((c) => c.kind === "cape")}
                textures={textures}
                equippedId={equipped?.cape ?? null}
                canEquip={!!account}
                busy={busy}
                onImport={() => void importFile("cape")}
                onPreview={(c) => setPreview((p) => ({ ...p, cape: textures[c.id] ?? null }))}
                onEquip={(id) => void equip("cape", id)}
                onRemove={(id) => void run("remove", async () => refresh(await api.removeCosmetic(id)))}
              />
              {account?.kind === "microsoft" && <MojangCapes account={account} capes={mojangCapes} setCapes={setMojangCapes} run={run} busy={busy} />}
            </>
          )}

          {tab === "catalog" && (
            <>
              <div>
                <h3 className="mb-1 text-sm font-semibold">Capas ({CAPES.length})</h3>
                <p className="mb-3 text-xs text-muted">Originais da Aurora. Também viram a textura da élitra.</p>
                <div className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-2">
                  {CAPES.map((c) => (
                    <CatalogCard
                      key={c.id}
                      name={c.name}
                      image={capePreview(c.id)}
                      busy={busy === `cape-${c.id}`}
                      onHover={() => setPreview((p) => ({ ...p, cape: capeTexture(c.id) }))}
                      onAdd={() => void addCape(c.id)}
                      action={account ? "Equipar" : "Adicionar"}
                    />
                  ))}
                </div>
              </div>
              <div>
                <h3 className="mb-1 text-sm font-semibold">Acessórios de cabeça ({ACCESSORIES.length})</h3>
                <p className="mb-3 text-xs text-muted">
                  Ficam na camada externa da skin, então aparecem em qualquer servidor, até sem mods. São aplicados na skin
                  equipada e salvos como uma skin nova.
                </p>
                <div className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-2">
                  {ACCESSORIES.map((a) => (
                    <CatalogCard
                      key={a.id}
                      name={a.name}
                      image={accessoryPreview(a, equippedSkin ? textures[equippedSkin.id] : undefined)}
                      head
                      busy={busy === `acc-${a.id}`}
                      onAdd={() => void addAccessory(a)}
                      action="Aplicar"
                    />
                  ))}
                </div>
              </div>
            </>
          )}

          {tab === "community" && (
            <CommunityGallery
              enabled={hasServer}
              notify={notify}
              onImported={(w) => void refresh(w)}
              onPreview={(item) => setPreview((p) => ({ ...p, skin: { tex: item.texture, model: item.model } }))}
            />
          )}

          {tab === "ears" && <EarsPanel />}
        </div>
      </section>
    </div>
  );
}

// ---------------------------------------------------------------- catálogo

const capeCache = new Map<string, string>();
function capeTexture(id: string): string {
  if (!capeCache.has(id)) capeCache.set(id, imageToDataUrl(renderCape(CAPES.find((c) => c.id === id)!)));
  return capeCache.get(id)!;
}

/** Recorte só da parte de trás da capa (10x16) ampliado, para o card. */
function capePreview(id: string): string {
  const key = `prev-${id}`;
  if (!capeCache.has(key)) {
    const src = renderCape(CAPES.find((c) => c.id === id)!);
    const c = document.createElement("canvas");
    c.width = 10;
    c.height = 16;
    const tmp = document.createElement("canvas");
    tmp.width = 64;
    tmp.height = 32;
    tmp.getContext("2d")!.putImageData(src, 0, 0);
    c.getContext("2d")!.drawImage(tmp, 1, 1, 10, 16, 0, 0, 10, 16);
    capeCache.set(key, c.toDataURL());
  }
  return capeCache.get(key)!;
}

const accCache = new Map<string, string>();
function accessoryPreview(acc: Accessory, base?: string): string {
  const key = `${acc.id}-${base?.length ?? 0}`;
  if (!accCache.has(key)) {
    const img = generateCharacter(DEFAULT_CHARACTER);
    clearHeadOverlay(img);
    acc.draw(img);
    accCache.set(key, imageToDataUrl(img));
  }
  return accCache.get(key)!;
}

function CatalogCard(props: {
  name: string;
  image: string;
  head?: boolean;
  busy: boolean;
  action: string;
  onAdd: () => void;
  onHover?: () => void;
}) {
  return (
    <div onMouseEnter={props.onHover} className="rounded-xl border border-line bg-panel p-2 transition hover:border-accent">
      <div className="grid h-24 place-items-center rounded-lg bg-bg">
        {props.head ? (
          <SkinHead src={props.image} size={64} />
        ) : (
          <img src={props.image} className="h-20" style={{ imageRendering: "pixelated" }} />
        )}
      </div>
      <p className="mt-1.5 truncate text-xs font-medium">{props.name}</p>
      <button
        disabled={props.busy}
        onClick={props.onAdd}
        className="mt-1.5 flex w-full items-center justify-center gap-1 rounded-md bg-accent py-1 text-xs font-semibold text-white transition hover:bg-accent-hover disabled:opacity-50"
      >
        {props.busy ? <Spinner size={12} /> : <Plus size={12} />} {props.action}
      </button>
    </div>
  );
}

// ---------------------------------------------------------------- comunidade

function CommunityGallery({
  enabled,
  notify,
  onImported,
  onPreview,
}: {
  enabled: boolean;
  notify: Props["notify"];
  onImported: (w: Wardrobe) => void;
  onPreview: (item: GalleryItem) => void;
}) {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<GalleryItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState<string | null>(null);
  const req = useRef(0);

  const load = useCallback(
    async (offset: number) => {
      const id = ++req.current;
      setLoading(true);
      try {
        const page = await api.galleryList(query, offset);
        if (id !== req.current) return;
        setItems((prev) => (offset === 0 ? page.items : [...prev, ...page.items]));
        setTotal(page.total);
      } catch (e) {
        if (id === req.current) notify("error", errorMessage(e));
      } finally {
        if (id === req.current) setLoading(false);
      }
    },
    [query, notify],
  );

  useEffect(() => {
    if (!enabled) return;
    const t = setTimeout(() => void load(0), 300);
    return () => clearTimeout(t);
  }, [enabled, load]);

  if (!enabled) {
    return (
      <div className="rounded-xl border border-line bg-panel p-5 text-sm">
        <p className="font-semibold">Galeria da comunidade</p>
        <p className="mt-1 text-muted">
          Para compartilhar e baixar skins de outros jogadores, suba o <code>cosmetics-server</code> (já incluído no projeto) na sua
          VPS e coloque a URL em <b>Configurações → Servidor de cosméticos</b>. Todos que usarem o mesmo servidor veem as mesmas
          skins.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="relative">
        <Search size={15} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Buscar por nome ou autor…"
          className="w-full rounded-md border border-line bg-bg py-2 pl-9 pr-3 text-sm outline-none focus:border-accent"
        />
      </div>
      <p className="text-xs text-muted">
        {loading ? "Carregando…" : `${total} skins compartilhadas`} · para compartilhar uma sua, use o botão{" "}
        <Share2 size={11} className="inline" /> em Meu guarda-roupa
      </p>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-2">
        {items.map((item) => (
          <div key={item.id} onMouseEnter={() => onPreview(item)} className="rounded-xl border border-line bg-panel p-2 transition hover:border-accent">
            <div className="grid h-24 place-items-center rounded-lg bg-bg">
              <SkinHead src={item.texture} size={64} />
            </div>
            <p className="mt-1.5 truncate text-xs font-medium">{item.name}</p>
            <p className="truncate text-[11px] text-muted">
              por {item.author} · <Download size={10} className="inline" /> {formatNumber(item.downloads)}
            </p>
            <button
              disabled={importing !== null}
              onClick={async () => {
                setImporting(item.id);
                try {
                  onImported(await api.galleryImport(item.id));
                  notify("success", `"${item.name}" adicionada ao seu guarda-roupa.`);
                } catch (e) {
                  notify("error", errorMessage(e));
                } finally {
                  setImporting(null);
                }
              }}
              className="mt-1.5 flex w-full items-center justify-center gap-1 rounded-md bg-accent py-1 text-xs font-semibold text-white hover:bg-accent-hover disabled:opacity-50"
            >
              {importing === item.id ? <Spinner size={12} /> : <Download size={12} />} Pegar
            </button>
          </div>
        ))}
      </div>
      {items.length < total && (
        <div className="text-center">
          <Button variant="subtle" disabled={loading} onClick={() => void load(items.length)}>
            Carregar mais
          </Button>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- Ears

function EarsPanel() {
  return (
    <div className="space-y-3 rounded-xl border border-line bg-panel p-5 text-sm">
      <p className="flex items-center gap-2 text-base font-semibold">
        <Feather size={18} className="text-accent" /> Asas, orelhas, rabo, chifres e focinho
      </p>
      <p className="text-muted">
        Esses cosméticos 3D usam o mod <b>Ears</b>, que o launcher instala sozinho junto com o CustomSkinLoader nas instâncias
        Fabric/NeoForge. As partes ficam guardadas <b>dentro da própria skin</b>, então aparecem para qualquer jogador que também
        tenha o Ears, até em conta pirata e sem servidor próprio.
      </p>
      <ol className="list-decimal space-y-1 pl-5 text-muted">
        <li>Abra o Manipulator do Ears e carregue a sua skin (exporte do guarda-roupa ou use o PNG original).</li>
        <li>Escolha orelhas, rabo, asas, chifres, focinho e cores.</li>
        <li>Baixe o PNG gerado e importe em Meu guarda-roupa → Importar PNG. Depois é só equipar.</li>
      </ol>
      <Button variant="primary" onClick={() => void openUrl("https://unascribed.com/ears")}>
        <ExternalLink size={15} /> Abrir o Manipulator do Ears
      </Button>
      <p className="text-xs text-muted">
        Pets e itens 3D presos ao corpo (mochila, auréola flutuante) exigiriam um mod próprio do launcher e não estão incluídos.
      </p>
    </div>
  );
}

// ---------------------------------------------------------------- componentes de lista

function MojangCapes({
  account,
  capes,
  setCapes,
  run,
  busy,
}: {
  account: Account;
  capes: MojangCape[] | null;
  setCapes: (c: MojangCape[]) => void;
  run: (label: string, fn: () => Promise<void>) => Promise<void>;
  busy: string | null;
}) {
  return (
    <div>
      <h3 className="mb-2 text-sm font-semibold">Capas oficiais da sua conta</h3>
      {capes === null ? (
        <Spinner size={14} />
      ) : capes.length === 0 ? (
        <p className="text-xs text-muted">Esta conta não tem capas oficiais.</p>
      ) : (
        <div className="flex flex-wrap gap-2">
          {[{ id: "", alias: "Nenhuma", state: capes.some((c) => c.state === "ACTIVE") ? "INACTIVE" : "ACTIVE", url: "" }, ...capes].map((c) => (
            <button
              key={c.id || "none"}
              disabled={busy !== null}
              onClick={() =>
                void run("mcape", async () => {
                  await api.setMojangCape(account.id, c.id || null);
                  setCapes(await api.mojangCapes(account.id));
                })
              }
              className={cx(
                "rounded-lg border px-3 py-2 text-xs font-medium transition",
                c.state === "ACTIVE" ? "border-ok bg-ok/10 text-ok" : "border-line bg-panel hover:border-accent",
              )}
            >
              {c.alias ?? c.id.slice(0, 8)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function SkinPreview({ skin, model, cape, items3d = [] }: { skin?: string; model: SkinModel; cape?: string; items3d?: string[] }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const viewer = useRef<SkinViewer | null>(null);
  const items = useRef<Cosmetics3DController | null>(null);
  const [walking, setWalking] = useState(true);
  const [elytra, setElytra] = useState(false);

  useEffect(() => {
    if (!canvas.current) return;
    const v = new SkinViewer({ canvas: canvas.current, width: 290, height: 380 });
    v.autoRotate = true;
    v.autoRotateSpeed = 0.6;
    v.zoom = 0.85;
    viewer.current = v;
    items.current = new Cosmetics3DController(v);
    return () => {
      items.current?.dispose();
      items.current = null;
      v.dispose();
      viewer.current = null;
    };
  }, []);

  useEffect(() => {
    if (viewer.current && skin) void viewer.current.loadSkin(skin, { model: model === "slim" ? "slim" : "default" });
  }, [skin, model]);

  useEffect(() => {
    const v = viewer.current;
    if (!v) return;
    if (cape) void v.loadCape(cape, { backEquipment: elytra ? "elytra" : "cape" });
    else v.loadCape(null);
  }, [cape, elytra]);

  const items3dKey = items3d.join(",");
  useEffect(() => {
    items.current?.set(items3dKey ? items3dKey.split(",") : []);
  }, [items3dKey]);

  useEffect(() => {
    if (viewer.current) viewer.current.animation = walking ? new WalkingAnimation() : new IdleAnimation();
  }, [walking]);

  return (
    <div className="relative mx-auto">
      <canvas ref={canvas} className="cursor-grab active:cursor-grabbing" />
      <div className="absolute bottom-1 right-1 flex gap-1">
        <IconToggle title={walking ? "Parar" : "Andar"} onClick={() => setWalking((w) => !w)}>
          {walking ? <Pause size={14} /> : <Play size={14} />}
        </IconToggle>
        {cape && (
          <IconToggle title={elytra ? "Ver como capa" : "Ver como élitra"} onClick={() => setElytra((e) => !e)}>
            <RotateCw size={14} />
          </IconToggle>
        )}
      </div>
    </div>
  );
}

function Items3DPanel(props: {
  equipped: string[];
  canEquip: boolean;
  busy: string | null;
  onToggle: (id: string) => void;
  onClear: () => void;
}) {
  const [status, setStatus] = useState<CosmeticsModStatus | null>(null);
  useEffect(() => {
    api.cosmeticsModStatus().then(setStatus).catch(() => setStatus(null));
  }, []);

  return (
    <div className="space-y-5">
      <div className="rounded-xl border border-accent/40 bg-accent/10 p-3 text-xs leading-relaxed text-zinc-300">
        <p className="mb-1 font-semibold text-white">Itens 3D de verdade, fora da skin</p>
        Asas que batem, auréola girando, pets flutuando do seu lado. São modelos 3D desenhados pelo mod{" "}
        <b>Aurora Cosmetics</b>, que o launcher instala sozinho em instâncias <b>Fabric {status?.minecraftVersion ?? "26.3"}</b>.
        Um item por espaço (asas, cabeça, costas e pet). Outros jogadores veem os seus itens se também usarem o Aurora e o
        mesmo servidor de cosméticos.
        {status && !status.built && (
          <p className="mt-2 text-warn">
            O mod ainda não foi compilado neste PC. Rode <b>compilar-mod.bat</b> na pasta do launcher uma vez.
          </p>
        )}
        {status?.built && <p className="mt-2 text-ok">Mod pronto: ele entra na instância quando você clicar em Jogar.</p>}
      </div>

      {SLOTS.map((slot) => {
        const list = COSMETICS_3D.filter((c) => c.slot === slot);
        return (
          <div key={slot}>
            <h3 className="mb-2 text-sm font-semibold">
              {SLOT_LABEL[slot]} ({list.length})
            </h3>
            <div className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-2">
              {list.map((item) => {
                const on = props.equipped.includes(item.id);
                const colors = [...new Set(item.parts.flatMap((p) => p.cubes.map((c) => c.c)))].slice(0, 5);
                return (
                  <button
                    key={item.id}
                    disabled={props.busy !== null}
                    onClick={() => props.onToggle(item.id)}
                    className={cx(
                      "flex flex-col items-start gap-2 rounded-xl border p-3 text-left text-sm transition disabled:opacity-60",
                      on ? "border-accent bg-accent/15" : "border-line bg-panel hover:border-accent/60",
                    )}
                  >
                    <div className="flex w-full items-center justify-between">
                      <span className="flex gap-1">
                        {colors.map((c) => (
                          <span key={c} className="h-3 w-3 rounded-sm" style={{ background: c }} />
                        ))}
                      </span>
                      {props.busy === `3d-${item.id}` ? <Spinner size={13} /> : on && <Check size={14} className="text-accent" />}
                    </div>
                    <span className="font-medium">{item.name}</span>
                    <span className="text-[11px] text-muted">{on ? "Equipado · clique para tirar" : props.canEquip ? "Equipar" : "Ver no preview"}</span>
                  </button>
                );
              })}
            </div>
          </div>
        );
      })}

      {props.equipped.length > 0 && (
        <Button variant="subtle" onClick={props.onClear}>
          <X size={15} /> Tirar todos os itens 3D
        </Button>
      )}
    </div>
  );
}

function IconToggle({ children, title, onClick }: { children: ReactNode; title: string; onClick: () => void }) {
  return (
    <button title={title} onClick={onClick} className="rounded-md bg-black/50 p-1.5 text-zinc-300 backdrop-blur transition hover:text-white">
      {children}
    </button>
  );
}

function Gallery(props: {
  title: string;
  kind: CosmeticKind;
  items: Cosmetic[];
  textures: Record<string, string>;
  equippedId: string | null;
  canEquip: boolean;
  busy: string | null;
  onImport: () => void;
  onCreate?: () => void;
  onPreview: (c: Cosmetic) => void;
  onEquip: (id: string | null) => void;
  onRemove: (id: string) => void;
  onEdit?: (c: Cosmetic) => void;
  onShare?: (c: Cosmetic) => void;
  onModel?: (id: string, m: SkinModel) => void;
}) {
  const { items, textures, equippedId } = props;
  return (
    <div>
      <div className="mb-2 flex items-center gap-2">
        <h3 className="text-sm font-semibold">{props.title}</h3>
        <span className="text-xs text-muted">{items.length}</span>
        {equippedId && props.canEquip && (
          <button onClick={() => props.onEquip(null)} className="ml-2 flex items-center gap-1 text-xs text-muted hover:text-white">
            <X size={12} /> Desequipar
          </button>
        )}
        <div className="ml-auto flex gap-2">
          {props.onCreate && (
            <Button variant="subtle" onClick={props.onCreate}>
              <Pencil size={14} /> Criar
            </Button>
          )}
          <Button variant="subtle" disabled={props.busy !== null} onClick={props.onImport}>
            <Plus size={14} /> Importar PNG
          </Button>
        </div>
      </div>
      {items.length === 0 ? (
        <button
          onClick={props.onCreate ?? props.onImport}
          className="flex w-full flex-col items-center gap-1 rounded-xl border border-dashed border-line py-8 text-sm text-muted transition hover:border-accent hover:text-white"
        >
          <ImageIcon size={22} />
          {props.kind === "skin" ? "Crie seu personagem ou importe uma skin 64x64" : "Pegue uma no Catálogo Aurora ou importe um PNG 64x32"}
        </button>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(8.5rem,1fr))] gap-2">
          {items.map((c) => {
            const isOn = c.id === equippedId;
            const tex = textures[c.id];
            return (
              <div
                key={c.id}
                onMouseEnter={() => props.onPreview(c)}
                className={cx(
                  "group relative overflow-hidden rounded-xl border bg-panel p-2 transition",
                  isOn ? "border-ok ring-1 ring-ok/40" : "border-line hover:border-accent",
                )}
              >
                <div className="grid h-24 place-items-center rounded-lg bg-bg">
                  {!tex ? (
                    <Spinner size={14} />
                  ) : props.kind === "skin" ? (
                    <SkinHead src={tex} size={64} />
                  ) : (
                    <img src={tex} className="max-h-20 max-w-full" style={{ imageRendering: "pixelated" }} />
                  )}
                </div>
                <p className="mt-1.5 truncate text-xs font-medium" title={c.name}>
                  {c.name}
                </p>
                <div className="mt-1 flex items-center gap-0.5">
                  {props.onModel && (
                    <button
                      onClick={() => props.onModel!(c.id, c.model === "slim" ? "classic" : "slim")}
                      className="rounded bg-panel-3 px-1.5 py-0.5 text-[10px] font-semibold uppercase text-muted hover:text-white"
                      title="Braço fino (3px) ou normal (4px)"
                    >
                      {c.model === "slim" ? "Fino" : "Normal"}
                    </button>
                  )}
                  <span className="ml-auto flex opacity-0 transition group-hover:opacity-100">
                    {props.onEdit && (
                      <button title="Editar" onClick={() => props.onEdit!(c)} className="rounded p-1 text-muted hover:text-white">
                        <Pencil size={12} />
                      </button>
                    )}
                    {props.onShare && (
                      <button title="Compartilhar na comunidade" onClick={() => props.onShare!(c)} className="rounded p-1 text-muted hover:text-accent">
                        <Share2 size={12} />
                      </button>
                    )}
                    <button title="Remover" onClick={() => props.onRemove(c.id)} className="rounded p-1 text-muted hover:text-err">
                      <Trash2 size={12} />
                    </button>
                  </span>
                </div>
                <button
                  disabled={!props.canEquip || isOn}
                  onClick={() => props.onEquip(c.id)}
                  className={cx(
                    "mt-1.5 w-full rounded-md py-1 text-xs font-semibold transition",
                    isOn ? "bg-ok/15 text-ok" : "bg-accent text-white hover:bg-accent-hover disabled:opacity-40",
                  )}
                >
                  {isOn ? "Equipado" : "Equipar"}
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
