// Mapa de UV da skin 64x64, gerador de personagem, acessórios de cabeça e capas originais.
// Tudo gerado aqui (nenhuma arte de terceiros), então pode ser distribuído livremente.

export type Rect = [x: number, y: number, w: number, h: number];
export type FaceId = "top" | "bottom" | "right" | "front" | "left" | "back";
export type Faces = Record<FaceId, Rect>;
export type PartId = "head" | "body" | "rightArm" | "leftArm" | "rightLeg" | "leftLeg";
export type Layer = "base" | "overlay";
export type Model = "classic" | "slim";
export type RGBA = [number, number, number, number];

export const SIZE = 64;

/** Layout padrão de um cubo no UV do Minecraft (origem x,y; largura w; altura h; profundidade d). */
function box(x: number, y: number, w: number, h: number, d: number): Faces {
  return {
    top: [x + d, y, w, d],
    bottom: [x + d + w, y, w, d],
    right: [x, y + d, d, h],
    front: [x + d, y + d, w, h],
    left: [x + d + w, y + d, d, h],
    back: [x + 2 * d + w, y + d, w, h],
  };
}

export const PART_LABEL: Record<PartId, string> = {
  head: "Cabeça",
  body: "Tronco",
  rightArm: "Braço direito",
  leftArm: "Braço esquerdo",
  rightLeg: "Perna direita",
  leftLeg: "Perna esquerda",
};

export function partFaces(part: PartId, layer: Layer, model: Model): Faces {
  const aw = model === "slim" ? 3 : 4;
  const o = layer === "overlay";
  switch (part) {
    case "head":
      return o ? box(32, 0, 8, 8, 8) : box(0, 0, 8, 8, 8);
    case "body":
      return o ? box(16, 32, 8, 12, 4) : box(16, 16, 8, 12, 4);
    case "rightArm":
      return o ? box(40, 32, aw, 12, 4) : box(40, 16, aw, 12, 4);
    case "leftArm":
      return o ? box(48, 48, aw, 12, 4) : box(32, 48, aw, 12, 4);
    case "rightLeg":
      return o ? box(0, 32, 4, 12, 4) : box(0, 16, 4, 12, 4);
    case "leftLeg":
      return o ? box(0, 48, 4, 12, 4) : box(16, 48, 4, 12, 4);
  }
}

export const PARTS: PartId[] = ["head", "body", "rightArm", "leftArm", "rightLeg", "leftLeg"];

/** Máscara 64x64 com 1 nos pixels que pertencem à parte/camada. */
export function partMask(part: PartId | "all", layer: Layer | "both", model: Model): Uint8Array {
  const mask = new Uint8Array(SIZE * SIZE);
  const parts = part === "all" ? PARTS : [part];
  const layers: Layer[] = layer === "both" ? ["base", "overlay"] : [layer];
  for (const p of parts)
    for (const l of layers)
      for (const [x, y, w, h] of Object.values(partFaces(p, l, model)))
        for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) mask[j * SIZE + i] = 1;
  return mask;
}

// ---------------------------------------------------------------- utilitários de pixel

export function hexToRgba(hex: string, a = 255): RGBA {
  const h = hex.replace("#", "");
  const n = parseInt(h.length === 3 ? h.split("").map((c) => c + c).join("") : h, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, a];
}

export function rgbaToHex([r, g, b]: RGBA): string {
  return "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
}

function shade([r, g, b, a]: RGBA, f: number): RGBA {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v * f)));
  return [c(r), c(g), c(b), a];
}

/** Ruído determinístico (mesma skin sempre gera o mesmo resultado). */
function noise(x: number, y: number, seed = 1): number {
  const n = Math.sin(x * 127.1 + y * 311.7 + seed * 74.7) * 43758.5453;
  return n - Math.floor(n);
}

export function setPx(img: ImageData, x: number, y: number, c: RGBA) {
  if (x < 0 || y < 0 || x >= img.width || y >= img.height) return;
  const i = (y * img.width + x) * 4;
  img.data[i] = c[0];
  img.data[i + 1] = c[1];
  img.data[i + 2] = c[2];
  img.data[i + 3] = c[3];
}

export function getPx(img: ImageData, x: number, y: number): RGBA {
  const i = (y * img.width + x) * 4;
  return [img.data[i]!, img.data[i + 1]!, img.data[i + 2]!, img.data[i + 3]!];
}

function fillRect(img: ImageData, [x, y, w, h]: Rect, color: RGBA | ((i: number, j: number) => RGBA)) {
  for (let j = 0; j < h; j++)
    for (let i = 0; i < w; i++) setPx(img, x + i, y + j, typeof color === "function" ? color(i, j) : color);
}

function textured(base: RGBA, x0: number, y0: number, amount = 0.08, seed = 1) {
  return (i: number, j: number) => shade(base, 1 - amount / 2 + noise(x0 + i, y0 + j, seed) * amount);
}

function fillPart(img: ImageData, part: PartId, layer: Layer, model: Model, color: RGBA, seed = 1) {
  for (const r of Object.values(partFaces(part, layer, model))) fillRect(img, r, textured(color, r[0], r[1], 0.1, seed));
}

export function blankImage(): ImageData {
  return new ImageData(SIZE, SIZE);
}

// ---------------------------------------------------------------- gerador de personagem

export interface CharacterOptions {
  skin: string;
  hair: string;
  eyes: string;
  shirt: string;
  pants: string;
  shoes: string;
  hairStyle: "curto" | "longo" | "careca";
  model: Model;
}

export const DEFAULT_CHARACTER: CharacterOptions = {
  skin: "#c58c6b",
  hair: "#3b2416",
  eyes: "#3d6bd8",
  shirt: "#5865f2",
  pants: "#2b2f45",
  shoes: "#1c1c22",
  hairStyle: "curto",
  model: "classic",
};

export function generateCharacter(o: CharacterOptions): ImageData {
  const img = blankImage();
  const skin = hexToRgba(o.skin);
  const hair = hexToRgba(o.hair);
  const shirt = hexToRgba(o.shirt);
  const pants = hexToRgba(o.pants);
  const shoes = hexToRgba(o.shoes);
  const m = o.model;

  // Cabeça
  fillPart(img, "head", "base", m, skin, 2);
  const head = partFaces("head", "base", m);
  const hairRows = o.hairStyle === "careca" ? 0 : o.hairStyle === "longo" ? 5 : 2;
  if (o.hairStyle !== "careca") {
    fillRect(img, head.top, textured(hair, 8, 0, 0.15, 3));
    for (const f of ["front", "right", "left", "back"] as FaceId[]) {
      const [x, y, w] = head[f];
      const rows = f === "back" ? Math.max(hairRows + 2, 6) : f === "front" ? Math.min(hairRows, 2) : hairRows + 1;
      fillRect(img, [x, y, w, rows], textured(hair, x, y, 0.15, 3));
    }
  }
  // Rosto
  const [fx, fy] = head.front;
  const white: RGBA = [245, 245, 245, 255];
  const eye = hexToRgba(o.eyes);
  setPx(img, fx + 1, fy + 4, white);
  setPx(img, fx + 2, fy + 4, eye);
  setPx(img, fx + 5, fy + 4, eye);
  setPx(img, fx + 6, fy + 4, white);
  setPx(img, fx + 3, fy + 6, shade(skin, 0.7));
  setPx(img, fx + 4, fy + 6, shade(skin, 0.7));
  setPx(img, fx + 3, fy + 5, shade(skin, 0.88));
  setPx(img, fx + 4, fy + 5, shade(skin, 0.88));

  // Tronco
  fillPart(img, "body", "base", m, shirt, 4);
  const body = partFaces("body", "base", m);
  fillRect(img, [body.front[0] + 3, body.front[1], 2, 1], shade(skin, 0.95)); // gola

  // Braços: manga curta + pele
  for (const arm of ["rightArm", "leftArm"] as PartId[]) {
    fillPart(img, arm, "base", m, skin, 5);
    const f = partFaces(arm, "base", m);
    fillRect(img, f.top, textured(shirt, f.top[0], f.top[1]));
    for (const k of ["front", "right", "left", "back"] as FaceId[]) {
      const [x, y, w] = f[k];
      fillRect(img, [x, y, w, 4], textured(shirt, x, y));
    }
  }

  // Pernas: calça + tênis
  for (const leg of ["rightLeg", "leftLeg"] as PartId[]) {
    fillPart(img, leg, "base", m, pants, 6);
    const f = partFaces(leg, "base", m);
    fillRect(img, f.bottom, textured(shoes, f.bottom[0], f.bottom[1]));
    for (const k of ["front", "right", "left", "back"] as FaceId[]) {
      const [x, y, w, h] = f[k];
      fillRect(img, [x, y + h - 2, w, 2], textured(shoes, x, y + h - 2));
    }
  }
  return img;
}

// ---------------------------------------------------------------- acessórios de cabeça (camada externa)

export interface Accessory {
  id: string;
  name: string;
  draw: (img: ImageData) => void;
}

const headO = () => partFaces("head", "overlay", "classic");

function ring(img: ImageData, rows: [number, number], color: RGBA | ((i: number, j: number) => RGBA)) {
  const f = headO();
  for (const k of ["front", "right", "left", "back"] as FaceId[]) {
    const [x, y, w] = f[k];
    fillRect(img, [x, y + rows[0], w, rows[1] - rows[0]], color);
  }
}

export const ACCESSORIES: Accessory[] = [
  {
    id: "coroa",
    name: "Coroa real",
    draw: (img) => {
      const gold: RGBA = [244, 196, 48, 255];
      ring(img, [0, 2], (i, j) => (j === 0 && i % 2 === 1 ? [0, 0, 0, 0] : shade(gold, 0.85 + noise(i, j, 7) * 0.3)));
      const [fx, fy] = headO().front;
      setPx(img, fx + 3, fy + 1, [220, 40, 60, 255]);
      setPx(img, fx + 4, fy + 1, [60, 120, 230, 255]);
    },
  },
  {
    id: "cavaleiro",
    name: "Capacete de cavaleiro",
    draw: (img) => {
      const iron: RGBA = [170, 176, 186, 255];
      const f = headO();
      for (const k of Object.keys(f) as FaceId[]) fillRect(img, f[k], textured(iron, f[k][0], f[k][1], 0.18, 9));
      const [fx, fy] = f.front;
      fillRect(img, [fx + 1, fy + 3, 6, 1], [20, 20, 26, 255]);
      for (const i of [2, 5]) setPx(img, fx + i, fy + 6, [40, 40, 48, 255]);
    },
  },
  {
    id: "bone",
    name: "Boné",
    draw: (img) => {
      const c: RGBA = [214, 48, 49, 255];
      const f = headO();
      fillRect(img, f.top, textured(c, f.top[0], f.top[1]));
      ring(img, [0, 2], textured(c, 40, 8));
      const [fx, fy] = f.front;
      fillRect(img, [fx, fy + 2, 8, 1], shade(c, 0.7));
      setPx(img, fx + 3, fy, [255, 255, 255, 255]);
      setPx(img, fx + 4, fy, [255, 255, 255, 255]);
    },
  },
  {
    id: "fones",
    name: "Fones gamer",
    draw: (img) => {
      const band: RGBA = [30, 30, 36, 255];
      const glow: RGBA = [88, 101, 242, 255];
      const f = headO();
      fillRect(img, [f.top[0] + 3, f.top[1], 2, 8], band);
      for (const k of ["right", "left"] as FaceId[]) {
        const [x, y] = f[k];
        fillRect(img, [x + 3, y, 2, 3], band);
        fillRect(img, [x + 1, y + 3, 6, 4], band);
        fillRect(img, [x + 2, y + 4, 4, 2], glow);
      }
    },
  },
  {
    id: "oculos",
    name: "Óculos escuros",
    draw: (img) => {
      const [fx, fy] = headO().front;
      const black: RGBA = [15, 15, 20, 255];
      fillRect(img, [fx, fy + 3, 8, 1], black);
      fillRect(img, [fx + 1, fy + 4, 2, 1], [35, 35, 50, 255]);
      fillRect(img, [fx + 5, fy + 4, 2, 1], [35, 35, 50, 255]);
      const f = headO();
      setPx(img, f.right[0] + 7, f.right[1] + 3, black);
      setPx(img, f.left[0], f.left[1] + 3, black);
    },
  },
  {
    id: "oculos3d",
    name: "Óculos 3D",
    draw: (img) => {
      const [fx, fy] = headO().front;
      fillRect(img, [fx, fy + 3, 8, 1], [240, 240, 240, 255]);
      fillRect(img, [fx + 1, fy + 4, 2, 1], [230, 40, 40, 255]);
      fillRect(img, [fx + 5, fy + 4, 2, 1], [40, 160, 240, 255]);
      fillRect(img, [fx, fy + 4, 1, 1], [240, 240, 240, 255]);
      fillRect(img, [fx + 3, fy + 4, 2, 1], [240, 240, 240, 255]);
      fillRect(img, [fx + 7, fy + 4, 1, 1], [240, 240, 240, 255]);
    },
  },
  {
    id: "bruxo",
    name: "Chapéu de bruxo",
    draw: (img) => {
      const p: RGBA = [92, 45, 145, 255];
      const f = headO();
      fillRect(img, f.top, textured(p, f.top[0], f.top[1], 0.2));
      ring(img, [0, 3], textured(p, 40, 8, 0.2));
      ring(img, [2, 3], [40, 20, 60, 255]);
      const [fx, fy] = f.front;
      fillRect(img, [fx + 3, fy + 2, 2, 1], [244, 196, 48, 255]);
    },
  },
  {
    id: "natal",
    name: "Gorro de Natal",
    draw: (img) => {
      const red: RGBA = [200, 30, 40, 255];
      const f = headO();
      fillRect(img, f.top, textured(red, f.top[0], f.top[1]));
      ring(img, [0, 2], textured(red, 40, 8));
      ring(img, [2, 3], [245, 245, 245, 255]);
      fillRect(img, [f.back[0] + 3, f.back[1] + 3, 2, 2], [245, 245, 245, 255]);
    },
  },
  {
    id: "ninja",
    name: "Bandana ninja",
    draw: (img) => {
      const c: RGBA = [30, 30, 36, 255];
      ring(img, [1, 3], textured(c, 40, 8, 0.15));
      const f = headO();
      fillRect(img, [f.back[0] + 3, f.back[1] + 3, 2, 3], c);
      const [fx, fy] = f.front;
      fillRect(img, [fx + 3, fy + 1, 2, 1], [170, 176, 186, 255]);
    },
  },
  {
    id: "mascara",
    name: "Máscara de gás",
    draw: (img) => {
      const g: RGBA = [70, 78, 70, 255];
      const f = headO();
      const [fx, fy] = f.front;
      fillRect(img, [fx, fy + 3, 8, 5], textured(g, fx, fy, 0.2, 4));
      fillRect(img, [fx + 1, fy + 3, 2, 2], [120, 200, 220, 255]);
      fillRect(img, [fx + 5, fy + 3, 2, 2], [120, 200, 220, 255]);
      fillRect(img, [fx + 3, fy + 6, 2, 2], [30, 30, 30, 255]);
      for (const k of ["right", "left"] as FaceId[]) fillRect(img, [f[k][0], f[k][1] + 4, 8, 1], [40, 40, 40, 255]);
    },
  },
  {
    id: "chifres",
    name: "Chifres de diabinho",
    draw: (img) => {
      const r: RGBA = [190, 30, 30, 255];
      const f = headO();
      const [tx, ty] = f.top;
      for (const [x, y] of [
        [1, 2],
        [2, 2],
        [1, 3],
        [5, 2],
        [6, 2],
        [6, 3],
      ])
        setPx(img, tx + x!, ty + y!, r);
      const [fx, fy] = f.front;
      setPx(img, fx + 1, fy, r);
      setPx(img, fx + 6, fy, r);
    },
  },
];

export function clearHeadOverlay(img: ImageData) {
  for (const r of Object.values(headO())) fillRect(img, r, [0, 0, 0, 0]);
}

// ---------------------------------------------------------------- capas (64x32, também viram élitra)

export interface CapeDesign {
  id: string;
  name: string;
  /** cor do pixel em coordenadas normalizadas (u,v ∈ [0,1]) da área da capa */
  color: (u: number, v: number, x: number, y: number) => RGBA;
}

const lerp = (a: RGBA, b: RGBA, t: number): RGBA => [0, 1, 2, 3].map((i) => Math.round(a[i]! + (b[i]! - a[i]!) * t)) as RGBA;

export const CAPES: CapeDesign[] = [
  { id: "aurora", name: "Aurora", color: (u, v) => lerp([88, 101, 242, 255], [217, 70, 239, 255], (u + v) / 2) },
  {
    id: "fogo",
    name: "Fogo",
    color: (_u, v, x, y) => lerp([255, 214, 0, 255], [200, 20, 10, 255], Math.min(1, v + noise(x, y, 3) * 0.35 - 0.1)),
  },
  {
    id: "oceano",
    name: "Oceano",
    color: (u, v) => lerp([20, 60, 140, 255], [60, 200, 230, 255], (Math.sin(u * 12 + v * 6) + 1) / 2 * 0.6 + v * 0.4),
  },
  {
    id: "galaxia",
    name: "Galáxia",
    color: (u, _v, x, y) =>
      noise(x, y, 11) > 0.93 ? [255, 255, 255, 255] : lerp([12, 10, 40, 255], [70, 20, 110, 255], noise(x >> 2, y >> 2, 5) * 0.7 + u * 0.3),
  },
  { id: "xadrez", name: "Xadrez", color: (_u, _v, x, y) => ((x >> 1) + (y >> 1)) % 2 ? [20, 20, 24, 255] : [235, 235, 235, 255] },
  {
    id: "brasil",
    name: "Brasil",
    color: (u, v) => {
      const d = Math.abs(u - 0.5) + Math.abs(v - 0.5);
      if ((u - 0.5) ** 2 + (v - 0.5) ** 2 < 0.03) return [30, 60, 150, 255];
      return d < 0.42 ? [254, 221, 0, 255] : [0, 156, 59, 255];
    },
  },
  {
    id: "arcoiris",
    name: "Arco-íris",
    color: (_u, v) => {
      const c: RGBA[] = [
        [228, 3, 3, 255],
        [255, 140, 0, 255],
        [255, 237, 0, 255],
        [0, 128, 38, 255],
        [0, 77, 255, 255],
        [117, 7, 135, 255],
      ];
      return c[Math.min(5, Math.floor(v * 6))]!;
    },
  },
  {
    id: "floresta",
    name: "Floresta",
    color: (_u, v, x, y) => shade(lerp([30, 90, 40, 255], [90, 160, 60, 255], noise(x, y, 8)), 1 - v * 0.3),
  },
  {
    id: "neve",
    name: "Neve",
    color: (_u, v, x, y) => (noise(x, y, 13) > 0.88 ? [255, 255, 255, 255] : lerp([200, 230, 250, 255], [120, 170, 220, 255], v)),
  },
  {
    id: "lava",
    name: "Lava",
    color: (_u, _v, x, y) => (noise(x, y, 17) > 0.75 ? [255, 120, 20, 255] : lerp([40, 10, 10, 255], [110, 20, 10, 255], noise(x >> 1, y >> 1, 2))),
  },
  {
    id: "ouro",
    name: "Ouro real",
    color: (u, v, x, y) => {
      const border = u < 0.1 || u > 0.9 || v < 0.06 || v > 0.94;
      return border ? [120, 20, 30, 255] : shade([244, 196, 48, 255], 0.85 + noise(x, y, 21) * 0.3);
    },
  },
  {
    id: "cyber",
    name: "Cyber",
    color: (_u, _v, x, y) => (x % 4 === 0 || y % 4 === 0 ? [0, 255, 200, 255] : [10, 12, 20, 255]),
  },
];

/** Desenha a capa em 64x32: área da capa (0..22 x 0..17) e da élitra (22..46 x 0..22). */
export function renderCape(design: CapeDesign): ImageData {
  const img = new ImageData(64, 32);
  const area = (x0: number, y0: number, w: number, h: number) => {
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) setPx(img, x0 + x, y0 + y, design.color(x / (w - 1), y / (h - 1), x0 + x, y0 + y));
  };
  area(0, 0, 22, 17);
  area(22, 0, 24, 22);
  return img;
}

// ---------------------------------------------------------------- conversões

export function imageToDataUrl(img: ImageData): string {
  const c = document.createElement("canvas");
  c.width = img.width;
  c.height = img.height;
  c.getContext("2d")!.putImageData(img, 0, 0);
  return c.toDataURL("image/png");
}

export async function dataUrlToImage(url: string): Promise<ImageData> {
  const el = new Image();
  el.src = url;
  await el.decode();
  const c = document.createElement("canvas");
  c.width = SIZE;
  c.height = SIZE;
  const ctx = c.getContext("2d")!;
  ctx.drawImage(el, 0, 0);
  // Skins antigas 64x32: espelha braço e perna direitos para os esquerdos
  if (el.height === 32) {
    const img = ctx.getImageData(0, 0, SIZE, SIZE);
    for (const [from, to] of [
      ["rightArm", "leftArm"],
      ["rightLeg", "leftLeg"],
    ] as [PartId, PartId][])
      mirrorLimb(img, from, to, "base", "classic");
    return img;
  }
  return ctx.getImageData(0, 0, SIZE, SIZE);
}

/** Copia um membro para o outro lado espelhando horizontalmente (como o próprio jogo faz). */
export function mirrorLimb(img: ImageData, from: PartId, to: PartId, layer: Layer, model: Model) {
  const a = partFaces(from, layer, model);
  const b = partFaces(to, layer, model);
  const map: [FaceId, FaceId][] = [
    ["top", "top"],
    ["bottom", "bottom"],
    ["front", "front"],
    ["back", "back"],
    ["right", "left"],
    ["left", "right"],
  ];
  const src = new ImageData(new Uint8ClampedArray(img.data), SIZE, SIZE);
  for (const [fa, fb] of map) {
    const [ax, ay, aw, ah] = a[fa];
    const [bx, by] = b[fb];
    for (let j = 0; j < ah; j++) for (let i = 0; i < aw; i++) setPx(img, bx + (aw - 1 - i), by + j, getPx(src, ax + i, ay + j));
  }
}

/** Desenha o rosto (frente da cabeça + camada externa) num canvas. */
export function drawFace(ctx: CanvasRenderingContext2D, skin: HTMLImageElement | HTMLCanvasElement, size: number) {
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, size, size);
  ctx.drawImage(skin, 8, 8, 8, 8, 0, 0, size, size);
  ctx.drawImage(skin, 40, 8, 8, 8, 0, 0, size, size);
}
