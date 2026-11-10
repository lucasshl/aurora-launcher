"""Gera os cosméticos 3D (voxel) do Aurora Launcher.

Saída: um único cosmetics.json usado pelo mod (no jogo) e pelo preview 3D do launcher.
Coordenadas em pixels no espaço do modelo do Minecraft: y para baixo, frente = -z.
  head: cabeça ocupa x -4..4, y -8..0, z -4..4 (origem no pescoço)
  body: tronco ocupa x -4..4, y 0..12, z -2..2 (costas em z=+2)
  root: origem no pescoço, gira junto com o corpo (usado por pets)
Animações: flapL, flapR (asas), spin, bob, float.
"""
import json
import math
import os
import sys

OUT = sys.argv[1:] or [os.path.join(os.path.dirname(__file__), "..", "src", "main", "resources", "assets", "aurora_cosmetics", "cosmetics.json")]


def runs(mask, palette, x0, y0, z0, z1, mirror=False, xdir=1):
    """Converte um desenho ASCII (linhas = y) em cubos, juntando pixels iguais na horizontal."""
    cubes = []
    for row, line in enumerate(mask):
        col = 0
        while col < len(line):
            ch = line[col]
            if ch in (" ", "."):
                col += 1
                continue
            start = col
            while col < len(line) and line[col] == ch:
                col += 1
            a = x0 + xdir * start
            b = x0 + xdir * col
            lo, hi = min(a, b), max(a, b)
            cubes.append({"f": [lo, y0 + row, z0], "t": [hi, y0 + row + 1, z1], "c": palette[ch]})
    return cubes


def box(f, t, c):
    return {"f": list(f), "t": list(t), "c": c}


def wing_pair(id_, name, mask, palette, thickness=1, y0=-5, spread=25):
    """Asa esquerda (x+) e direita (x-) a partir do mesmo desenho; a coluna 0 fica junto às costas."""
    left = {"pivot": [1, 2, 2], "rot": [0, -spread, 0], "anim": "flapL",
            "cubes": runs(mask, palette, 1, y0, 2, 2 + thickness, xdir=1)}
    right = {"pivot": [-1, 2, 2], "rot": [0, spread, 0], "anim": "flapR",
             "cubes": runs(mask, palette, -1, y0, 2, 2 + thickness, xdir=-1)}
    return {"id": id_, "name": name, "slot": "wings", "bone": "body", "parts": [left, right]}


ANGEL = [
    "aa.............",
    "aaaa...........",
    "aaaaaa.........",
    "baaaaaaa.......",
    "bbaaaaaaaa.....",
    "bbbaaaaaaaaa...",
    "bbbbaaaaaaaaaa.",
    ".bbbbaaaaaaaaaa",
    ".bbbbbaaaaaaaa.",
    "..bbbbbaaaaaa..",
    "..bbbbbbaaaa...",
    "...bbbbbbaa....",
    "....bbbbbb.....",
    ".....bbbb......",
    "......bb.......",
]
DEMON = [
    "d..............",
    "dd.............",
    "dmd...........d",
    "dmmd........ddd",
    "dmmmd.....ddmd.",
    "dmmmmd..ddmmd..",
    "dmmmmmddmmmmd..",
    "dmmmmmmmmmmd...",
    "dmmmmmmmmmmd...",
    ".dmmmmmmmmd....",
    ".dmmmdmmmmd....",
    "..dmd.dmmd.....",
    "..dd...dd......",
]
BUTTERFLY = [
    "..ooo.......",
    ".oyyyoo.....",
    "oyyyyyyoo...",
    "oyykyyyyyo..",
    "oyyyyyyyyyo.",
    ".oyyyyyyyyo.",
    "..ooooooooo.",
    "..obbbbo....",
    ".obbkbbbo...",
    ".obbbbbbo...",
    "..obbbbo....",
    "...oooo.....",
]
FIRE = [
    "r.............",
    "ro............",
    "roo.....y.....",
    "rooo...yy.....",
    "roooo.yoy..y..",
    "rooooooooyyy..",
    "rooooooooooyy.",
    ".roooooooooy..",
    ".rrooooooy....",
    "..rroooyy.....",
    "...rrrooy.....",
    "....rrry......",
]
CRYSTAL = [
    "c.............",
    "cl............",
    "cll...........",
    "clll......l...",
    "cllll....ll...",
    "clllll..lll...",
    "cllllllllll...",
    ".clllllllll...",
    ".cclllllll....",
    "..cclllll.....",
    "...ccclll.....",
    ".....cc.......",
]
DRAGON = [
    "g.............",
    "gg............",
    "gsg..........g",
    "gssg.......ggs",
    "gsssg....ggssg",
    "gssssg..gsssg.",
    "gsssssggsssg..",
    "gssssssssssg..",
    ".gsssssssssg..",
    ".gssgsssssg...",
    "..gg.gsssg....",
    "......ggg.....",
]

items = [
    wing_pair("asas_anjo", "Asas de anjo", ANGEL, {"a": "#f8f8ff", "b": "#d9dcf0"}),
    wing_pair("asas_demonio", "Asas de demônio", DEMON, {"d": "#2a0a0e", "m": "#8b1a24"}),
    wing_pair("asas_dragao", "Asas de dragão", DRAGON, {"g": "#1f5c2a", "s": "#4caf50"}),
    wing_pair("asas_borboleta", "Asas de borboleta", BUTTERFLY,
              {"o": "#1b1b1b", "y": "#ffb300", "b": "#29b6f6", "k": "#fafafa"}, y0=-4, spread=15),
    wing_pair("asas_fogo", "Asas de fogo", FIRE, {"r": "#b71c1c", "o": "#ff6d00", "y": "#ffd600"}),
    wing_pair("asas_cristal", "Asas de cristal", CRYSTAL, {"c": "#4dd0e1", "l": "#b2ebf2"}),
]

# ------------------------------------------------------------------ cabeça

def ring(cx, cz, r, y, c, h=1):
    cubes = []
    for i in range(24):
        a = 2 * math.pi * i / 24
        x = round(cx + r * math.cos(a))
        z = round(cz + r * math.sin(a))
        cubes.append(box((x, y, z), (x + 1, y + h, z + 1), c))
    # remove duplicados
    seen, out = set(), []
    for cb in cubes:
        k = tuple(cb["f"])
        if k not in seen:
            seen.add(k)
            out.append(cb)
    return out

items.append({"id": "aureola", "name": "Auréola", "slot": "head", "bone": "head", "parts": [
    {"pivot": [0, -11, 0], "anim": "spin", "cubes": ring(-0.5, -0.5, 4, -11, "#ffd54f")}]})

items.append({"id": "coroa3d", "name": "Coroa 3D", "slot": "head", "bone": "head", "parts": [
    {"pivot": [0, 0, 0], "cubes": [
        box((-4.5, -9, -4.5), (4.5, -8, 4.5), "#f9a825"),
        *[box((x, -11, -4.5), (x + 1, -9, -3.5), "#fbc02d") for x in (-4.5, -1.5, 1.5, 3.5)],
        *[box((x, -11, 3.5), (x + 1, -9, 4.5), "#fbc02d") for x in (-4.5, -1.5, 1.5, 3.5)],
        *[box((-4.5, -11, z), (-3.5, -9, z + 1), "#fbc02d") for z in (-1.5, 1.5)],
        *[box((3.5, -11, z), (4.5, -9, z + 1), "#fbc02d") for z in (-1.5, 1.5)],
        box((-0.5, -10, -5), (0.5, -9, -4.5), "#e53935"),
    ]}]})

items.append({"id": "cartola", "name": "Cartola", "slot": "head", "bone": "head", "parts": [
    {"pivot": [0, 0, 0], "cubes": [
        box((-6, -9, -6), (6, -8, 6), "#1a1a1a"),
        box((-3.5, -16, -3.5), (3.5, -9, 3.5), "#212121"),
        box((-3.6, -11, -3.6), (3.6, -10, 3.6), "#c62828"),
    ]}]})

witch = [box((-6, -9, -6), (6, -8, 6), "#4a148c")]
for i, (w, h) in enumerate([(4.5, 2), (3.5, 2), (2.5, 2), (1.5, 2), (0.8, 2)]):
    y = -10 - i * 2
    witch.append(box((-w, y - h + 1, -w + i * 0.6), (w, y + 1, w + i * 0.6), "#6a1b9a"))
witch.append(box((-4.6, -10, -4.6), (4.6, -9, 4.6), "#ffb300"))
items.append({"id": "chapeu_bruxo3d", "name": "Chapéu de bruxo 3D", "slot": "head", "bone": "head",
              "parts": [{"pivot": [0, 0, 0], "cubes": witch}]})

items.append({"id": "orelhas_gato", "name": "Orelhas de gato", "slot": "head", "bone": "head", "parts": [
    {"pivot": [0, 0, 0], "cubes": [
        box((-4, -10, -1), (-1, -8, 0), "#3e2723"), box((-3.5, -11, -1), (-1.5, -10, 0), "#3e2723"),
        box((-3, -10, -1.2), (-2, -8.5, -1), "#f48fb1"),
        box((1, -10, -1), (4, -8, 0), "#3e2723"), box((1.5, -11, -1), (3.5, -10, 0), "#3e2723"),
        box((2, -10, -1.2), (3, -8.5, -1), "#f48fb1"),
    ]}]})

items.append({"id": "orelhas_coelho", "name": "Orelhas de coelho", "slot": "head", "bone": "head", "parts": [
    {"pivot": [-2, -8, 0], "rot": [0, 0, -10], "cubes": [box((-3.5, -16, -0.5), (-1.5, -8, 0.5), "#fafafa"),
                                                       box((-3, -15, -0.7), (-2, -9, -0.5), "#f8bbd0")]},
    {"pivot": [2, -8, 0], "rot": [0, 0, 10], "cubes": [box((1.5, -16, -0.5), (3.5, -8, 0.5), "#fafafa"),
                                                     box((2, -15, -0.7), (3, -9, -0.5), "#f8bbd0")]},
]})

items.append({"id": "chifres_viking", "name": "Capacete viking", "slot": "head", "bone": "head", "parts": [
    {"pivot": [0, 0, 0], "cubes": [
        box((-4.6, -8.6, -4.6), (4.6, -5, 4.6), "#9e9e9e"),
        box((-0.5, -9.2, -4.8), (0.5, -5, 4.8), "#757575"),
        box((-7, -7, -1), (-4.6, -6, 1), "#efebe9"), box((-8, -10, -1), (-7, -6, 1), "#efebe9"),
        box((-8, -11, -0.5), (-7.5, -10, 0.5), "#d7ccc8"),
        box((4.6, -7, -1), (7, -6, 1), "#efebe9"), box((7, -10, -1), (8, -6, 1), "#efebe9"),
        box((7.5, -11, -0.5), (8, -10, 0.5), "#d7ccc8"),
    ]}]})

# ------------------------------------------------------------------ costas

items.append({"id": "mochila", "name": "Mochila", "slot": "back", "bone": "body", "parts": [
    {"pivot": [0, 0, 0], "cubes": [
        box((-3.5, 1, 2), (3.5, 10, 5), "#6d4c41"),
        box((-3, 6, 5), (3, 9.5, 6), "#5d4037"),
        box((-0.5, 5.5, 5), (0.5, 6.5, 6.2), "#ffca28"),
        box((-3.6, 0.5, 1.6), (-2.6, 9, 2.1), "#3e2723"), box((2.6, 0.5, 1.6), (3.6, 9, 2.1), "#3e2723"),
    ]}]})

items.append({"id": "katana", "name": "Katana nas costas", "slot": "back", "bone": "body", "parts": [
    {"pivot": [0, 6, 3], "rot": [0, 0, 40], "cubes": [
        box((-0.5, -6, 2.5), (0.5, 12, 3.2), "#cfd8dc"),
        box((-1.5, 12, 2.3), (1.5, 13, 3.4), "#ffd54f"),
        box((-0.6, 13, 2.4), (0.6, 18, 3.3), "#212121"),
    ]}]})

items.append({"id": "jetpack", "name": "Jetpack", "slot": "back", "bone": "body", "parts": [
    {"pivot": [0, 0, 0], "cubes": [
        box((-3.5, 1, 2), (-0.5, 10, 5), "#90a4ae"), box((0.5, 1, 2), (3.5, 10, 5), "#90a4ae"),
        box((-3, 0, 2.5), (-1, 1, 4.5), "#e53935"), box((1, 0, 2.5), (3, 1, 4.5), "#e53935"),
        box((-3, 10, 2.5), (-1, 11, 4.5), "#455a64"), box((1, 10, 2.5), (3, 11, 4.5), "#455a64"),
    ]},
    {"pivot": [0, 12, 3.5], "anim": "bob", "cubes": [
        box((-2.7, 11, 2.8), (-1.3, 13, 4.2), "#ff9100"), box((1.3, 11, 2.8), (2.7, 13, 4.2), "#ff9100"),
        box((-2.4, 13, 3.1), (-1.6, 14, 3.9), "#ffea00"), box((1.6, 13, 3.1), (2.4, 14, 3.9), "#ffea00"),
    ]}]})

# ------------------------------------------------------------------ pets (flutuam ao lado da cabeça)

def at(cubes, dx, dy, dz):
    return [{"f": [c["f"][0] + dx, c["f"][1] + dy, c["f"][2] + dz],
             "t": [c["t"][0] + dx, c["t"][1] + dy, c["t"][2] + dz], "c": c["c"]} for c in cubes]


def pet(id_, name, cubes):
    return {"id": id_, "name": name, "slot": "pet", "bone": "root",
            "parts": [{"pivot": [11, -6, 3], "anim": "float", "cubes": at(cubes, 11, -6, 3)}]}

def at(cubes, dx, dy, dz):
    return [{"f": [c["f"][0] + dx, c["f"][1] + dy, c["f"][2] + dz], "t": [c["t"][0] + dx, c["t"][1] + dy, c["t"][2] + dz], "c": c["c"]} for c in cubes]

slime = [box((-2.5, -2.5, -2.5), (2.5, 2.5, 2.5), "#76c442"),
         box((-1.5, -1, -2.7), (-0.5, 0, -2.5), "#1b5e20"), box((0.5, -1, -2.7), (1.5, 0, -2.5), "#1b5e20"),
         box((-0.5, 1, -2.7), (0.5, 1.5, -2.5), "#1b5e20")]
ghost = [box((-2.5, -3, -2.5), (2.5, 2, 2.5), "#f5f5f5"),
         *[box((x, 2, -2.5), (x + 1, 3, 2.5), "#f5f5f5") for x in (-2.5, -0.5, 1.5)],
         box((-1.5, -1.5, -2.7), (-0.5, 0, -2.5), "#212121"), box((0.5, -1.5, -2.7), (1.5, 0, -2.5), "#212121")]
bee = [box((-2, -1.5, -2.5), (2, 1.5, 2.5), "#fdd835"),
       box((-2.05, -1.55, -0.5), (2.05, 1.55, 0.5), "#212121"), box((-2.05, -1.55, 1.5), (2.05, 1.55, 2.0), "#212121"),
       box((-1.5, -0.5, -2.7), (-0.5, 0.5, -2.5), "#212121"), box((0.5, -0.5, -2.7), (1.5, 0.5, -2.5), "#212121"),
       box((-3.5, -3, -0.5), (-0.5, -1.5, 1.5), "#e3f2fd"), box((0.5, -3, -0.5), (3.5, -1.5, 1.5), "#e3f2fd")]
orb = [box((-1.5, -1.5, -1.5), (1.5, 1.5, 1.5), "#7c4dff"), box((-1, -1, -1), (1, 1, 1), "#e1bee7"),
       box((-2.5, -0.3, -0.3), (2.5, 0.3, 0.3), "#b388ff"), box((-0.3, -2.5, -0.3), (0.3, 2.5, 0.3), "#b388ff")]
mini_dragon = [box((-1.5, -1.5, -3), (1.5, 1.5, 2), "#c62828"), box((-1.2, -3, -4.5), (1.2, -0.5, -2), "#c62828"),
               box((-1, -2.5, -4.7), (-0.4, -1.9, -4.5), "#ffeb3b"), box((0.4, -2.5, -4.7), (1, -1.9, -4.5), "#ffeb3b"),
               box((-0.5, -0.5, 2), (0.5, 0.5, 5), "#b71c1c"),
               box((-4.5, -2, -1), (-1.5, -1.5, 1.5), "#ef9a9a"), box((1.5, -2, -1), (4.5, -1.5, 1.5), "#ef9a9a")]

items += [
    pet("pet_slime", "Pet slime", slime),
    pet("pet_fantasma", "Pet fantasma", ghost),
    pet("pet_abelha", "Pet abelha", bee),
    pet("pet_orbe", "Orbe Aurora", orb),
    pet("pet_dragao", "Mini dragão", mini_dragon),
]

data = {"version": 1, "items": items}
for path in OUT:
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(data, fh, ensure_ascii=False, separators=(",", ":"))
print(f"{len(items)} cosméticos gerados")
