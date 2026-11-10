import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, LogOut, Palette, Plus, ShieldCheck, Shirt, WifiOff } from "lucide-react";
import type { Account, AccountsView, Wardrobe } from "../lib/types";
import { Avatar } from "./Avatar";
import { SkinHead } from "./SkinHead";
import { cx } from "./ui";

interface Props {
  view: AccountsView;
  wardrobe: Wardrobe;
  textures: Record<string, string>;
  onSelect: (id: string) => void;
  onRemove: (id: string) => void;
  onAdd: (tab: "microsoft" | "offline") => void;
  onEquipSkin: (cosmeticId: string) => void;
  onOpenWardrobe: () => void;
  onOpenEditor: () => void;
}

function KindBadge({ account }: { account: Account }) {
  return account.kind === "microsoft" ? (
    <span className="inline-flex items-center gap-1 text-[11px] font-medium text-ms">
      <ShieldCheck size={12} /> Original
    </span>
  ) : (
    <span className="inline-flex items-center gap-1 text-[11px] font-medium text-warn">
      <WifiOff size={12} /> Pirata
    </span>
  );
}

/** Avatar da conta: usa a skin equipada no guarda-roupa quando houver. */
export function AccountAvatar({
  account,
  wardrobe,
  textures,
  size,
}: {
  account: Account;
  wardrobe: Wardrobe;
  textures: Record<string, string>;
  size: number;
}) {
  const skinId = wardrobe.equipped[account.id]?.skin;
  const tex = skinId ? textures[skinId] : undefined;
  return tex ? <SkinHead src={tex} size={size} /> : <Avatar account={account} size={size} />;
}

export function AccountSwitcher(props: Props) {
  const { view, wardrobe, textures } = props;
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState({ top: 0, right: 0 });
  const btn = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const active = view.accounts.find((a) => a.id === view.active) ?? null;
  const skins = wardrobe.items.filter((c) => c.kind === "skin");
  const equippedSkin = active ? wardrobe.equipped[active.id]?.skin : undefined;

  // O menu é renderizado num portal (fora do cabeçalho com blur), posicionado sob o botão
  useLayoutEffect(() => {
    if (!open || !btn.current) return;
    const r = btn.current.getBoundingClientRect();
    setPos({ top: r.bottom + 8, right: window.innerWidth - r.right });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!panel.current?.contains(t) && !btn.current?.contains(t)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", esc);
    window.addEventListener("resize", () => setOpen(false), { once: true });
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);

  const groups: { title: string; list: Account[] }[] = [
    { title: "Contas originais", list: view.accounts.filter((a) => a.kind === "microsoft") },
    { title: "Contas piratas (offline)", list: view.accounts.filter((a) => a.kind === "offline") },
  ];

  return (
    <>
      <button
        ref={btn}
        onClick={() => (active ? setOpen((o) => !o) : props.onAdd("offline"))}
        className={cx(
          "flex items-center gap-2.5 rounded-lg py-1.5 pl-1.5 pr-3 transition",
          open ? "bg-panel-3" : "hover:bg-panel-2",
          !active && "bg-accent pl-3 font-semibold text-white hover:bg-accent-hover",
        )}
      >
        {active ? (
          <>
            <AccountAvatar account={active} wardrobe={wardrobe} textures={textures} size={30} />
            <span className="text-left leading-tight">
              <span className="block text-sm font-semibold">{active.username}</span>
              <KindBadge account={active} />
            </span>
            <ChevronDown size={16} className={cx("text-muted transition-transform", open && "rotate-180")} />
          </>
        ) : (
          <>
            <Plus size={16} /> Adicionar conta
          </>
        )}
      </button>

      {open &&
        createPortal(
          <div
            ref={panel}
            style={{ top: pos.top, right: pos.right }}
            className="fixed z-[70] w-80 animate-pop-in overflow-hidden rounded-xl border border-line bg-panel-2 shadow-2xl"
          >
            <div className="max-h-[70vh] overflow-y-auto">
              {groups.map(
                (g) =>
                  g.list.length > 0 && (
                    <div key={g.title} className="p-1.5">
                      <p className="px-2 pb-1 pt-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">{g.title}</p>
                      {g.list.map((acc) => (
                        <div key={acc.id} className="group flex items-center rounded-md hover:bg-panel-3">
                          <button
                            className="flex flex-1 items-center gap-2.5 px-2 py-2 text-left"
                            onClick={() => {
                              props.onSelect(acc.id);
                              setOpen(false);
                            }}
                          >
                            <AccountAvatar account={acc} wardrobe={wardrobe} textures={textures} size={28} />
                            <span className="flex-1 leading-tight">
                              <span className="block text-sm font-medium">{acc.username}</span>
                              <KindBadge account={acc} />
                            </span>
                            {acc.id === view.active && <Check size={16} className="text-ok" />}
                          </button>
                          <button
                            title="Remover conta"
                            onClick={() => props.onRemove(acc.id)}
                            className="mr-1.5 rounded p-1.5 text-muted opacity-0 transition hover:bg-err/15 hover:text-err group-hover:opacity-100"
                          >
                            <LogOut size={14} />
                          </button>
                        </div>
                      ))}
                    </div>
                  ),
              )}

              {/* Troca rápida de skin */}
              <div className="border-t border-line p-3">
                <div className="mb-2 flex items-center justify-between">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-muted">Minhas skins</p>
                  <button
                    onClick={() => {
                      setOpen(false);
                      props.onOpenWardrobe();
                    }}
                    className="text-[11px] font-medium text-accent hover:underline"
                  >
                    Ver todas
                  </button>
                </div>
                {skins.length === 0 ? (
                  <p className="text-xs text-muted">Nenhuma skin ainda. Crie uma no editor ou importe um PNG.</p>
                ) : (
                  <div className="grid grid-cols-6 gap-1.5">
                    {skins.slice(0, 12).map((s) => (
                      <button
                        key={s.id}
                        title={s.name}
                        onClick={() => props.onEquipSkin(s.id)}
                        className={cx(
                          "rounded-md p-0.5 ring-2 transition",
                          equippedSkin === s.id ? "ring-ok" : "ring-transparent hover:ring-accent",
                        )}
                      >
                        {textures[s.id] ? <SkinHead src={textures[s.id]!} size={36} /> : <div className="h-9 w-9 rounded-md bg-panel-3" />}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>

            <div className="grid grid-cols-2 gap-px border-t border-line bg-line">
              <MenuButton icon={ShieldCheck} label="Conta original" onClick={() => (setOpen(false), props.onAdd("microsoft"))} />
              <MenuButton icon={WifiOff} label="Conta pirata" onClick={() => (setOpen(false), props.onAdd("offline"))} />
              <MenuButton icon={Palette} label="Editar personagem" onClick={() => (setOpen(false), props.onOpenEditor())} />
              <MenuButton icon={Shirt} label="Guarda-roupa" onClick={() => (setOpen(false), props.onOpenWardrobe())} />
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}

function MenuButton({ icon: Icon, label, onClick }: { icon: typeof Plus; label: string; onClick: () => void }) {
  return (
    <button onClick={onClick} className="flex items-center gap-2 bg-panel-2 px-3 py-2.5 text-xs font-medium text-zinc-300 transition hover:bg-panel-3 hover:text-white">
      <Icon size={14} /> {label}
    </button>
  );
}
