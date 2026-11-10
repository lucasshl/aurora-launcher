// Cosméticos 3D do mod Aurora Cosmetics, montados com three.js sobre o modelo do skinview3d.
// O JSON é o mesmo empacotado no mod, e as animações seguem as mesmas fórmulas do CosmeticsLayer.java.
import { BoxGeometry, Euler, Group, Mesh, MeshLambertMaterial, type Object3D } from "three";
import type { SkinViewer } from "skinview3d";
import data from "./cosmetics3d.json";

export type Slot = "wings" | "head" | "back" | "pet";
export interface CubeDef {
  f: [number, number, number];
  t: [number, number, number];
  c: string;
}
export interface PartDef {
  pivot: [number, number, number];
  rot?: [number, number, number];
  anim?: string;
  cubes: CubeDef[];
}
export interface Cosmetic3D {
  id: string;
  name: string;
  slot: Slot;
  bone: "head" | "body" | "root";
  parts: PartDef[];
}

export const COSMETICS_3D: Cosmetic3D[] = (data as unknown as { items: Cosmetic3D[] }).items;
export const SLOT_LABEL: Record<Slot, string> = { wings: "Asas", head: "Cabeça", back: "Costas", pet: "Pets" };
export const SLOTS: Slot[] = ["wings", "head", "back", "pet"];

const DEG = Math.PI / 180;

interface Animated {
  anim: string;
  group: Group;
}

/**
 * Converte do espaço do modelo do Minecraft (y para baixo, frente = -z) para o do skinview3d
 * (y para cima, frente = +z): (x, y, z) → (x, -y, -z). O tronco do skinview3d tem origem no
 * centro (6 px abaixo do pescoço), por isso o deslocamento de +6 no osso "body".
 */
function convert(x: number, y: number, z: number, bone: Cosmetic3D["bone"]): [number, number, number] {
  return [x, -y + (bone === "body" ? 6 : 0), -z];
}

const materials = new Map<string, MeshLambertMaterial>();
function material(color: string) {
  if (!materials.has(color)) materials.set(color, new MeshLambertMaterial({ color }));
  return materials.get(color)!;
}

function build(item: Cosmetic3D): { root: Group; animated: Animated[] } {
  const root = new Group();
  root.name = `aurora3d:${item.id}`;
  const animated: Animated[] = [];
  for (const part of item.parts) {
    const [px, py, pz] = part.pivot;
    const pivot = new Group();
    pivot.position.set(...convert(px, py, pz, item.bone));
    const fixed = new Group();
    const [rx, ry, rz] = part.rot ?? [0, 0, 0];
    // Inverter y e z equivale a girar 180° em X: rotações em Y e Z trocam de sinal
    fixed.rotation.copy(new Euler(rx * DEG, -ry * DEG, -rz * DEG, "ZYX"));
    const anim = new Group();
    for (const c of part.cubes) {
      const w = c.t[0] - c.f[0];
      const h = c.t[1] - c.f[1];
      const d = c.t[2] - c.f[2];
      const mesh = new Mesh(new BoxGeometry(w, h, d), material(c.c));
      // centro do cubo relativo ao pivô, já convertido
      mesh.position.set((c.f[0] + c.t[0]) / 2 - px, -((c.f[1] + c.t[1]) / 2 - py), -((c.f[2] + c.t[2]) / 2 - pz));
      anim.add(mesh);
    }
    fixed.add(anim);
    pivot.add(fixed);
    root.add(pivot);
    if (part.anim && part.anim !== "none") animated.push({ anim: part.anim, group: anim });
  }
  return { root, animated };
}

function boneOf(viewer: SkinViewer, bone: Cosmetic3D["bone"]): Object3D {
  const skin = viewer.playerObject.skin;
  return bone === "head" ? skin.head : bone === "body" ? skin.body : skin;
}

/** Mantém os itens 3D equipados presos ao personagem do preview e anima a cada frame. */
export class Cosmetics3DController {
  private attached: { root: Group; parent: Object3D; animated: Animated[] }[] = [];
  private raf = 0;
  private start = performance.now();

  constructor(private viewer: SkinViewer) {
    const loop = () => {
      this.animate((performance.now() - this.start) / 50); // 1 tick = 50 ms
      this.raf = requestAnimationFrame(loop);
    };
    this.raf = requestAnimationFrame(loop);
  }

  set(ids: string[]) {
    for (const a of this.attached) a.parent.remove(a.root);
    this.attached = [];
    for (const id of ids) {
      const item = COSMETICS_3D.find((c) => c.id === id);
      if (!item) continue;
      const { root, animated } = build(item);
      const parent = boneOf(this.viewer, item.bone);
      parent.add(root);
      this.attached.push({ root, parent, animated });
    }
  }

  private animate(t: number) {
    for (const a of this.attached)
      for (const { anim, group } of a.animated) {
        switch (anim) {
          case "flapL":
          case "flapR": {
            const angle = Math.sin(t * 0.18) * 22;
            // Em Y o sinal inverte na conversão de eixos
            group.rotation.set(0, (anim === "flapL" ? angle : -angle) * DEG, 0);
            break;
          }
          case "spin":
            group.rotation.set(0, -((t * 4) % 360) * DEG, 0);
            break;
          case "bob":
            group.position.y = -Math.sin(t * 0.15) * 0.64;
            break;
          case "float":
            group.position.y = -Math.sin(t * 0.1) * 1.2;
            group.rotation.set(0, -Math.sin(t * 0.05) * 25 * DEG, 0);
            break;
        }
      }
  }

  dispose() {
    cancelAnimationFrame(this.raf);
    for (const a of this.attached) a.parent.remove(a.root);
    this.attached = [];
  }
}

/** Mantém no máximo um item por slot: equipar substitui o anterior do mesmo slot. */
export function toggleItem(current: string[], id: string): string[] {
  const item = COSMETICS_3D.find((c) => c.id === id);
  if (!item) return current;
  if (current.includes(id)) return current.filter((x) => x !== id);
  const others = current.filter((x) => COSMETICS_3D.find((c) => c.id === x)?.slot !== item.slot);
  return [...others, id];
}
