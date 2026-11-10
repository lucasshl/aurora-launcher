import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeft, Copy, Droplet, Eraser, FlipHorizontal, Layers, PaintBucket, Pencil, Pipette, Redo2, Save, Sparkles, Undo2, Wand2 } from "lucide-react";
import { SkinViewer, IdleAnimation } from "skinview3d";
import { api, errorMessage } from "../lib/api";
import {
  ACCESSORIES,
  DEFAULT_CHARACTER,
  PART_LABEL,
  PARTS,
  SIZE,
  clearHeadOverlay,
  dataUrlToImage,
  generateCharacter,
  getPx,
  hexToRgba,
  imageToDataUrl,
  mirrorLimb,
  partMask,
  rgbaToHex,
  setPx,
  type CharacterOptions,
  type Layer,
  type Model,
  type PartId,
  type RGBA,
} from "../lib/skin";
import type { Cosmetic } from "../lib/types";
import { Button, Field, Spinner, cx, inputClass } from "./ui";

type Tool = "pencil" | "eraser" | "fill" | "picker";
const ZOOM = 9;
const PALETTE = [
  "#000000", "#3b3b3b", "#7a7a7a", "#ffffff", "#c58c6b", "#8d5524", "#f1c27d", "#e0ac69",
  "#d62828", "#f77f00", "#fcbf49", "#2a9d8f", "#5865f2", "#7b2cbf", "#06d6a0", "#ef476f",
];

interface Props {
  /** Skin do guarda-roupa para editar (ou null = começar do gerador) */
  source: Cosmetic | null;
  sourceTexture?: string;
  onClose: () => void;
  onSaved: (c: Cosmetic, equip: boolean) => void;
  notify: (kind: "error" | "success" | "info", text: string) => void;
}

export function SkinEditor({ source, sourceTexture, onClose, onSaved, notify }: Props) {
  const tex = useRef<ImageData>(generateCharacter(DEFAULT_CHARACTER));
  const [version, setVersion] = useState(0); // força re-render quando a textura muda
  const undo = useRef<ImageData[]>([]);
  const redo = useRef<ImageData[]>([]);
  const [model, setModel] = useState<Model>(source?.model ?? "classic");
  const [part, setPart] = useState<PartId | "all">("all");
  const [layer, setLayer] = useState<Layer>("base");
  const [tool, setTool] = useState<Tool>("pencil");
  const [color, setColor] = useState("#5865f2");
  const [recent, setRecent] = useState<string[]>([]);
  const [name, setName] = useState(source?.name ?? "Meu personagem");
  const [character, setCharacter] = useState<CharacterOptions>(DEFAULT_CHARACTER);
  const [saving, setSaving] = useState(false);
  const canvas = useRef<HTMLCanvasElement>(null);
  const painting = useRef(false);

  const mask = useMemo(() => partMask(part, layer, model), [part, layer, model]);
  const bump = () => setVersion((v) => v + 1);

  // Carrega a skin escolhida (ou gera um personagem novo)
  useEffect(() => {
    if (sourceTexture) {
      dataUrlToImage(sourceTexture)
        .then((img) => {
          tex.current = img;
          bump();
        })
        .catch(() => notify("error", "Não foi possível abrir esta skin."));
    }
  }, [sourceTexture, notify]);

  const snapshot = () => {
    undo.current.push(new ImageData(new Uint8ClampedArray(tex.current.data), SIZE, SIZE));
    if (undo.current.length > 80) undo.current.shift();
    redo.current = [];
  };

  const history = (from: React.MutableRefObject<ImageData[]>, to: React.MutableRefObject<ImageData[]>) => {
    const prev = from.current.pop();
    if (!prev) return;
    to.current.push(new ImageData(new Uint8ClampedArray(tex.current.data), SIZE, SIZE));
    tex.current = prev;
    bump();
  };

  // Desenha a textura ampliada com grade e destaque da parte selecionada
  useEffect(() => {
    const ctx = canvas.current?.getContext("2d");
    if (!ctx) return;
    const img = tex.current;
    ctx.clearRect(0, 0, SIZE * ZOOM, SIZE * ZOOM);
    for (let y = 0; y < SIZE; y++)
      for (let x = 0; x < SIZE; x++) {
        const inside = mask[y * SIZE + x] === 1;
        const [r, g, b, a] = getPx(img, x, y);
        // xadrez para transparência
        ctx.fillStyle = (x + y) % 2 ? "#1a1c22" : "#14161b";
        ctx.fillRect(x * ZOOM, y * ZOOM, ZOOM, ZOOM);
        if (a > 0) {
          ctx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
          ctx.fillRect(x * ZOOM, y * ZOOM, ZOOM, ZOOM);
        }
        if (!inside) {
          ctx.fillStyle = "rgba(8,9,12,0.72)";
          ctx.fillRect(x * ZOOM, y * ZOOM, ZOOM, ZOOM);
        }
      }
    ctx.strokeStyle = "rgba(255,255,255,0.05)";
    ctx.lineWidth = 1;
    for (let i = 0; i <= SIZE; i++) {
      ctx.beginPath();
      ctx.moveTo(i * ZOOM + 0.5, 0);
      ctx.lineTo(i * ZOOM + 0.5, SIZE * ZOOM);
      ctx.moveTo(0, i * ZOOM + 0.5);
      ctx.lineTo(SIZE * ZOOM, i * ZOOM + 0.5);
      ctx.stroke();
    }
  }, [version, mask]);

  const allowed = (x: number, y: number) => x >= 0 && y >= 0 && x < SIZE && y < SIZE && mask[y * SIZE + x] === 1;

  const pickColor = (hex: string) => {
    setColor(hex);
    setRecent((r) => [hex, ...r.filter((c) => c !== hex)].slice(0, 8));
  };

  const floodFill = (sx: number, sy: number, c: RGBA) => {
    const img = tex.current;
    const target = getPx(img, sx, sy).join();
    if (target === c.join()) return;
    const stack = [[sx, sy]];
    const seen = new Uint8Array(SIZE * SIZE);
    while (stack.length) {
      const [x, y] = stack.pop()!;
      if (!allowed(x!, y!) || seen[y! * SIZE + x!]) continue;
      seen[y! * SIZE + x!] = 1;
      if (getPx(img, x!, y!).join() !== target) continue;
      setPx(img, x!, y!, c);
      stack.push([x! + 1, y!], [x! - 1, y!], [x!, y! + 1], [x!, y! - 1]);
    }
  };

  const apply = (e: React.PointerEvent<HTMLCanvasElement>, first: boolean) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = Math.floor(((e.clientX - rect.left) / rect.width) * SIZE);
    const y = Math.floor(((e.clientY - rect.top) / rect.height) * SIZE);
    if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) return;
    if (tool === "picker") {
      const px = getPx(tex.current, x, y);
      if (px[3] > 0) pickColor(rgbaToHex(px));
      setTool("pencil");
      return;
    }
    if (!allowed(x, y)) return;
    if (tool === "eraser" && layer === "base" && part !== "all") {
      if (first) notify("info", "A camada base não pode ficar transparente. Use a borracha na camada externa.");
      return;
    }
    if (first) snapshot();
    if (tool === "fill") floodFill(x, y, hexToRgba(color));
    else setPx(tex.current, x, y, tool === "eraser" ? [0, 0, 0, 0] : hexToRgba(color));
    bump();
  };

  const regenerate = (opts: CharacterOptions) => {
    snapshot();
    tex.current = generateCharacter(opts);
    setModel(opts.model);
    bump();
  };

  const save = async (equip: boolean) => {
    setSaving(true);
    try {
      const c = await api.importCosmeticData(imageToDataUrl(tex.current), "skin", name, model);
      onSaved(c, equip);
      notify("success", equip ? "Skin salva e equipada." : "Skin salva no guarda-roupa.");
    } catch (e) {
      notify("error", errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  // Atalhos: Ctrl+Z / Ctrl+Y, B/E/G/I para ferramentas
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).tagName === "INPUT") return;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        history(undo, redo);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
        e.preventDefault();
        history(redo, undo);
      } else if (e.key === "b") setTool("pencil");
      else if (e.key === "e") setTool("eraser");
      else if (e.key === "g") setTool("fill");
      else if (e.key === "i") setTool("picker");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const TOOLS: { id: Tool; icon: typeof Pencil; label: string }[] = [
    { id: "pencil", icon: Pencil, label: "Lápis (B)" },
    { id: "eraser", icon: Eraser, label: "Borracha (E)" },
    { id: "fill", icon: PaintBucket, label: "Balde (G)" },
    { id: "picker", icon: Pipette, label: "Conta-gotas (I)" },
  ];

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 p-5">
      <header className="flex items-center gap-3">
        <Button variant="ghost" onClick={onClose}>
          <ArrowLeft size={16} /> Voltar
        </Button>
        <h1 className="text-lg font-bold">Editor de personagem</h1>
        <input value={name} onChange={(e) => setName(e.target.value)} className={cx(inputClass, "ml-4 w-56")} placeholder="Nome da skin" />
        <div className="ml-auto flex gap-2">
          <Button variant="subtle" disabled={saving} onClick={() => void save(false)}>
            <Save size={15} /> Salvar
          </Button>
          <Button variant="primary" disabled={saving} onClick={() => void save(true)}>
            {saving ? <Spinner size={14} /> : <Sparkles size={15} />} Salvar e equipar
          </Button>
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[17rem_auto_1fr] gap-4">
        {/* Painel de ferramentas */}
        <aside className="min-h-0 space-y-4 overflow-y-auto rounded-xl border border-line bg-panel p-3">
          <section>
            <p className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">Parte do corpo</p>
            <div className="grid grid-cols-2 gap-1">
              {(["all", ...PARTS] as const).map((p) => (
                <button
                  key={p}
                  onClick={() => setPart(p)}
                  className={cx(
                    "rounded-md px-2 py-1.5 text-xs font-medium transition",
                    part === p ? "bg-accent text-white" : "bg-bg text-muted hover:text-zinc-200",
                  )}
                >
                  {p === "all" ? "Corpo todo" : PART_LABEL[p]}
                </button>
              ))}
            </div>
          </section>

          <section>
            <p className="mb-1.5 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">
              <Layers size={12} /> Camada
            </p>
            <div className="grid grid-cols-2 gap-1">
              {(["base", "overlay"] as Layer[]).map((l) => (
                <button
                  key={l}
                  onClick={() => setLayer(l)}
                  className={cx("rounded-md px-2 py-1.5 text-xs font-medium transition", layer === l ? "bg-accent text-white" : "bg-bg text-muted")}
                >
                  {l === "base" ? "Base (pele/roupa)" : "Externa (acessórios)"}
                </button>
              ))}
            </div>
          </section>

          <section>
            <p className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">Ferramentas</p>
            <div className="flex gap-1">
              {TOOLS.map((t) => (
                <button
                  key={t.id}
                  title={t.label}
                  onClick={() => setTool(t.id)}
                  className={cx("rounded-md p-2 transition", tool === t.id ? "bg-accent text-white" : "bg-bg text-muted hover:text-white")}
                >
                  <t.icon size={16} />
                </button>
              ))}
              <button title="Desfazer (Ctrl+Z)" onClick={() => history(undo, redo)} className="ml-auto rounded-md bg-bg p-2 text-muted hover:text-white">
                <Undo2 size={16} />
              </button>
              <button title="Refazer (Ctrl+Y)" onClick={() => history(redo, undo)} className="rounded-md bg-bg p-2 text-muted hover:text-white">
                <Redo2 size={16} />
              </button>
            </div>
          </section>

          <section>
            <p className="mb-1.5 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">
              <Droplet size={12} /> Cor
            </p>
            <div className="mb-2 flex items-center gap-2">
              <input type="color" value={color} onChange={(e) => pickColor(e.target.value)} className="h-9 w-12 cursor-pointer rounded bg-transparent" />
              <input value={color} onChange={(e) => /^#[0-9a-f]{6}$/i.test(e.target.value) && pickColor(e.target.value)} className={cx(inputClass, "font-mono text-xs")} />
            </div>
            <div className="grid grid-cols-8 gap-1">
              {[...recent, ...PALETTE.filter((c) => !recent.includes(c))].slice(0, 24).map((c) => (
                <button
                  key={c}
                  onClick={() => pickColor(c)}
                  style={{ background: c }}
                  className={cx("h-6 rounded ring-2", color === c ? "ring-white" : "ring-transparent")}
                />
              ))}
            </div>
          </section>

          <section>
            <p className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">Espelhar</p>
            <div className="grid grid-cols-2 gap-1">
              <Button variant="subtle" className="text-xs" onClick={() => (snapshot(), mirrorLimb(tex.current, "rightArm", "leftArm", layer, model), bump())}>
                <FlipHorizontal size={13} /> Braço D→E
              </Button>
              <Button variant="subtle" className="text-xs" onClick={() => (snapshot(), mirrorLimb(tex.current, "rightLeg", "leftLeg", layer, model), bump())}>
                <FlipHorizontal size={13} /> Perna D→E
              </Button>
            </div>
          </section>

          <section>
            <p className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">Acessórios de cabeça</p>
            <div className="grid grid-cols-2 gap-1">
              {ACCESSORIES.map((a) => (
                <button
                  key={a.id}
                  onClick={() => {
                    snapshot();
                    clearHeadOverlay(tex.current);
                    a.draw(tex.current);
                    bump();
                  }}
                  className="rounded-md bg-bg px-2 py-1.5 text-left text-xs text-zinc-300 transition hover:bg-panel-3 hover:text-white"
                >
                  {a.name}
                </button>
              ))}
              <button
                onClick={() => (snapshot(), clearHeadOverlay(tex.current), bump())}
                className="rounded-md bg-bg px-2 py-1.5 text-left text-xs text-err hover:bg-err/10"
              >
                Remover acessório
              </button>
            </div>
          </section>
        </aside>

        {/* Tela de pintura */}
        <section className="flex min-h-0 flex-col items-center gap-2 overflow-auto rounded-xl border border-line bg-[#0a0b0e] p-3">
          <canvas
            ref={canvas}
            width={SIZE * ZOOM}
            height={SIZE * ZOOM}
            style={{ width: SIZE * ZOOM, height: SIZE * ZOOM, imageRendering: "pixelated" }}
            className={cx("touch-none rounded", tool === "picker" ? "cursor-crosshair" : "cursor-cell")}
            onPointerDown={(e) => {
              painting.current = true;
              e.currentTarget.setPointerCapture(e.pointerId);
              apply(e, true);
            }}
            onPointerMove={(e) => painting.current && (tool === "pencil" || tool === "eraser") && apply(e, false)}
            onPointerUp={() => (painting.current = false)}
          />
          <p className="text-[11px] text-muted">
            Áreas escuras ficam bloqueadas para a parte/camada escolhida. Escolha "Corpo todo" para pintar livre.
          </p>
        </section>

        {/* Preview 3D + gerador */}
        <aside className="flex min-h-0 flex-col gap-3 overflow-y-auto">
          <Preview3D texture={tex.current} version={version} model={model} />
          <div className="flex gap-1 rounded-lg bg-bg p-1">
            {(["classic", "slim"] as Model[]).map((m) => (
              <button
                key={m}
                onClick={() => setModel(m)}
                className={cx("flex-1 rounded-md py-1.5 text-xs font-medium", model === m ? "bg-panel-3 text-white" : "text-muted")}
              >
                {m === "classic" ? "Braço normal (4px)" : "Braço fino (3px)"}
              </button>
            ))}
          </div>

          <div className="space-y-3 rounded-xl border border-line bg-panel p-3">
            <p className="flex items-center gap-1.5 text-sm font-semibold">
              <Wand2 size={15} className="text-accent" /> Criar personagem do zero
            </p>
            <div className="grid grid-cols-3 gap-2">
              {(
                [
                  ["skin", "Pele"],
                  ["hair", "Cabelo"],
                  ["eyes", "Olhos"],
                  ["shirt", "Camisa"],
                  ["pants", "Calça"],
                  ["shoes", "Tênis"],
                ] as const
              ).map(([k, label]) => (
                <label key={k} className="flex items-center gap-2 text-xs text-muted">
                  <input
                    type="color"
                    value={character[k]}
                    onChange={(e) => setCharacter((c) => ({ ...c, [k]: e.target.value }))}
                    className="h-7 w-8 cursor-pointer rounded bg-transparent"
                  />
                  {label}
                </label>
              ))}
            </div>
            <Field label="Cabelo">
              <select
                className={inputClass}
                value={character.hairStyle}
                onChange={(e) => setCharacter((c) => ({ ...c, hairStyle: e.target.value as CharacterOptions["hairStyle"] }))}
              >
                <option value="curto">Curto</option>
                <option value="longo">Longo</option>
                <option value="careca">Careca</option>
              </select>
            </Field>
            <Button variant="subtle" className="w-full" onClick={() => regenerate({ ...character, model })}>
              <Copy size={14} /> Gerar personagem (substitui o atual)
            </Button>
          </div>
        </aside>
      </div>
    </div>
  );
}

function Preview3D({ texture, version, model }: { texture: ImageData; version: number; model: Model }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const viewer = useRef<SkinViewer | null>(null);
  const work = useRef<HTMLCanvasElement>(document.createElement("canvas"));

  useEffect(() => {
    if (!ref.current) return;
    const v = new SkinViewer({ canvas: ref.current, width: 260, height: 340 });
    v.animation = new IdleAnimation();
    v.autoRotate = true;
    v.autoRotateSpeed = 0.5;
    v.zoom = 0.85;
    viewer.current = v;
    return () => v.dispose();
  }, []);

  const push = useCallback(() => {
    const c = work.current;
    c.width = SIZE;
    c.height = SIZE;
    c.getContext("2d")!.putImageData(texture, 0, 0);
    viewer.current?.loadSkin(c, { model: model === "slim" ? "slim" : "default" });
  }, [texture, model]);

  useEffect(() => {
    const id = requestAnimationFrame(push);
    return () => cancelAnimationFrame(id);
  }, [push, version]);

  return (
    <div className="grid place-items-center rounded-xl border border-line bg-gradient-to-b from-panel-2 to-bg">
      <canvas ref={ref} className="cursor-grab active:cursor-grabbing" />
    </div>
  );
}
