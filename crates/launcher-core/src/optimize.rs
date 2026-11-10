//! Otimização: pacotes curados de mods (desempenho e PvP), presets gráficos
//! para `options.txt` e flags de inicialização rápida da JVM (AppCDS).
//!
//! Por que mods e não "reescrever o jogo em Rust": o Minecraft roda inteiro na JVM.
//! Os ganhos reais vêm de mods que substituem o renderizador (Sodium), a lógica de
//! ticks (Lithium), a geração de chunks (C2ME/Noisium) e a inicialização (ModernFix).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::configs::merge_options;
use crate::content::{self, InstallReport};
use crate::download::{Downloader, ProgressSink};
use crate::error::{LauncherError, Result};
use crate::instance::InstanceConfig;
use crate::modrinth::{self, ContentKind};
use crate::paths::{write_atomic, LauncherPaths};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Render,
    Logic,
    Memory,
    Worldgen,
    Startup,
    Network,
    Pvp,
    Hud,
    Shaders,
    Utility,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CuratedMod {
    pub key: &'static str,
    /// Slugs alternativos em ordem de preferência (ex.: Sodium → Embeddium no Forge).
    pub candidates: &'static [&'static str],
    pub name: &'static str,
    pub description: &'static str,
    pub category: Category,
    pub recommended: bool,
}

macro_rules! curated {
    ($key:literal, [$($c:literal),+], $name:literal, $cat:ident, $rec:literal, $desc:literal) => {
        CuratedMod { key: $key, candidates: &[$($c),+], name: $name, description: $desc, category: Category::$cat, recommended: $rec }
    };
}

pub const PERFORMANCE: &[CuratedMod] = &[
    curated!(
        "sodium",
        ["sodium", "embeddium"],
        "Sodium",
        Render,
        true,
        "Novo motor de renderização. Costuma multiplicar o FPS em PCs fracos."
    ),
    curated!(
        "lithium",
        ["lithium"],
        "Lithium",
        Logic,
        true,
        "Otimiza física, IA de mobs e os ticks do mundo (inclusive no singleplayer)."
    ),
    curated!("ferrite-core", ["ferrite-core"], "FerriteCore", Memory, true, "Reduz bastante o uso de RAM."),
    curated!(
        "modernfix",
        ["modernfix"],
        "ModernFix",
        Startup,
        true,
        "Acelera a abertura do jogo e o carregamento de mundos."
    ),
    curated!(
        "c2me",
        ["c2me-fabric", "c2me-neoforge"],
        "C2ME",
        Worldgen,
        true,
        "Geração e carregamento de chunks em paralelo, usando todos os núcleos."
    ),
    curated!("noisium", ["noisium"], "Noisium", Worldgen, true, "Geração de terreno mais rápida."),
    curated!(
        "entityculling",
        ["entityculling"],
        "Entity Culling",
        Render,
        true,
        "Não desenha entidades e baús escondidos atrás de blocos."
    ),
    curated!(
        "immediatelyfast",
        ["immediatelyfast"],
        "ImmediatelyFast",
        Render,
        true,
        "Acelera HUD, textos, mapas e itens na tela."
    ),
    curated!("moreculling", ["moreculling"], "More Culling", Render, true, "Culling extra de folhas e blocos."),
    curated!(
        "badoptimizations",
        ["badoptimizations"],
        "BadOptimizations",
        Logic,
        true,
        "Cache de cálculos de luz, céu e partículas."
    ),
    curated!(
        "dynamic-fps",
        ["dynamic-fps"],
        "Dynamic FPS",
        Utility,
        true,
        "Reduz o FPS e a CPU quando o jogo está minimizado ou em segundo plano."
    ),
    curated!(
        "krypton",
        ["krypton", "krypton-fnp"],
        "Krypton",
        Network,
        true,
        "Otimiza a rede (menos lag em servidores)."
    ),
    curated!(
        "lazydfu",
        ["lazydfu"],
        "LazyDFU",
        Startup,
        true,
        "Pula a otimização do DataFixer na abertura (versões até 1.19)."
    ),
    curated!(
        "starlight",
        ["starlight"],
        "Starlight",
        Logic,
        true,
        "Motor de iluminação reescrito (versões até 1.20.1)."
    ),
    curated!(
        "sodium-extra",
        ["sodium-extra"],
        "Sodium Extra",
        Render,
        false,
        "Mais opções para PC fraco: desligar animações, partículas, neblina…"
    ),
    curated!(
        "reeses-sodium-options",
        ["reeses-sodium-options"],
        "Reese's Sodium Options",
        Utility,
        false,
        "Tela de opções do Sodium mais organizada."
    ),
    curated!(
        "chunky",
        ["chunky"],
        "Chunky",
        Worldgen,
        false,
        "Pré-gera o mapa (/chunky start). Depois os chunks carregam instantaneamente."
    ),
];

pub const PVP: &[CuratedMod] = &[
    curated!(
        "freelook",
        ["freelook", "freelook-og"],
        "Freelook",
        Pvp,
        true,
        "Câmera livre: olhe em volta sem virar o personagem (segure Alt)."
    ),
    curated!("zoom", ["zoomify", "ok-zoomer"], "Zoomify", Pvp, true, "Zoom suave estilo OptiFine (tecla C)."),
    curated!(
        "togglesprint",
        ["toggle-sprint-display", "toggle-sprint"],
        "Toggle Sprint",
        Pvp,
        true,
        "Correr e agachar no modo alternar, com indicador na tela."
    ),
    curated!("armorhud", ["ukus-armor-hud"], "Armor HUD", Hud, true, "Durabilidade da armadura e dos itens na tela."),
    curated!(
        "effects",
        ["statuseffecttimer"],
        "Status Effect Timer",
        Hud,
        true,
        "Tempo restante das poções sobre os ícones."
    ),
    curated!("appleskin", ["appleskin"], "AppleSkin", Hud, true, "Mostra saturação e quanto cada comida recupera."),
    curated!("betterf3", ["betterf3"], "BetterF3", Hud, false, "Tela F3 organizada e colorida."),
    curated!("iris", ["iris"], "Iris Shaders", Shaders, false, "Shaders compatíveis com o Sodium."),
];

pub fn find_curated(key: &str) -> Option<&'static CuratedMod> {
    PERFORMANCE.iter().chain(PVP.iter()).find(|m| m.key == key)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CuratedStatus {
    #[serde(flatten)]
    pub item: CuratedMod,
    /// Slug disponível para esta versão/loader (`None` = incompatível).
    pub available_slug: Option<String>,
    pub version: Option<String>,
    pub installed: bool,
}

/// Para cada mod curado, descobre qual candidato existe para a versão/loader da instância.
pub async fn curated_status(
    client: &reqwest::Client,
    paths: &LauncherPaths,
    instance: &InstanceConfig,
    list: &'static [CuratedMod],
) -> Result<Vec<CuratedStatus>> {
    if !instance.loader.supports_mods() {
        return Err(LauncherError::Loader("instâncias Vanilla não carregam mods; use Fabric ou NeoForge".into()));
    }
    let loaders = ContentKind::Mod.loaders(instance.loader);
    let installed: HashSet<String> = content::managed_project_ids(paths, &instance.id, ContentKind::Mod).await;
    let mc = instance.minecraft_version.as_str();

    // Cada future recebe dados próprios (sem empréstimos) para ser `Send` dentro de comandos Tauri
    let installed = std::sync::Arc::new(installed);
    let futures = list.iter().enumerate().map(|(i, item)| {
        let (client, loaders, installed, mc) = (client.clone(), loaders.clone(), installed.clone(), mc.to_owned());
        async move {
            for slug in item.candidates {
                if let Ok(Some(v)) = modrinth::best_version(&client, slug, &loaders, &mc).await {
                    return (
                        i,
                        CuratedStatus {
                            item: item.clone(),
                            available_slug: Some((*slug).to_owned()),
                            version: Some(v.version_number.clone()),
                            installed: installed.contains(&v.project_id),
                        },
                    );
                }
            }
            (i, CuratedStatus { item: item.clone(), available_slug: None, version: None, installed: false })
        }
    });
    let futures: Vec<_> = futures.collect();
    let mut out: Vec<(usize, CuratedStatus)> = stream::iter(futures).buffer_unordered(8).collect().await;
    out.sort_by_key(|(i, _)| *i);
    Ok(out.into_iter().map(|(_, s)| s).collect())
}

/// Instala os mods curados escolhidos (pelas chaves), resolvendo o candidato compatível.
pub async fn install_curated(
    paths: &LauncherPaths,
    downloader: &Downloader,
    instance: &InstanceConfig,
    keys: &[String],
    sink: &dyn ProgressSink,
) -> Result<InstallReport> {
    let loaders = ContentKind::Mod.loaders(instance.loader);
    let mut slugs = Vec::new();
    let mut skipped = Vec::new();
    for key in keys {
        let item = find_curated(key).ok_or_else(|| LauncherError::Other(format!("mod curado desconhecido: {key}")))?;
        let mut found = None;
        for slug in item.candidates {
            if modrinth::best_version(downloader.client(), slug, &loaders, &instance.minecraft_version).await?.is_some()
            {
                found = Some((*slug).to_owned());
                break;
            }
        }
        match found {
            Some(s) => slugs.push(s),
            None => skipped.push(item.name.to_owned()),
        }
    }
    let mut report =
        content::install_projects(paths, downloader, instance, ContentKind::Mod, &slugs, None, sink).await?;
    report.skipped.extend(skipped);
    Ok(report)
}

// ---------------------------------------------------------------- presets gráficos

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GraphicsPreset {
    /// PC muito fraco: prioriza ≥ 50 FPS acima de tudo.
    Potato,
    Balanced,
    Quality,
}

fn minor_version(mc: &str) -> u32 {
    let mut it = mc.split('.');
    match (it.next(), it.next()) {
        (Some("1"), Some(m)) => {
            m.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse().ok()).unwrap_or(99)
        }
        _ => 99, // snapshots e numeração nova (26.x): formato moderno
    }
}

/// Valores de `options.txt` compatíveis com o formato da versão.
pub fn preset_values(preset: GraphicsPreset, mc_version: &str) -> Vec<(&'static str, String)> {
    let minor = minor_version(mc_version);
    let (render, sim, fancy, ao, particles, shadows, blend, mipmap, clouds, fps, entity_dist) = match preset {
        GraphicsPreset::Potato => (5, 5, 0, false, 2, false, 0, 0, "false", 120, "0.5"),
        GraphicsPreset::Balanced => (8, 6, 0, true, 1, true, 1, 2, "fast", 144, "0.75"),
        GraphicsPreset::Quality => (12, 10, 1, true, 0, true, 2, 4, "true", 260, "1.0"),
    };
    let mut v: Vec<(&'static str, String)> = vec![
        ("renderDistance", render.to_string()),
        ("particles", particles.to_string()),
        ("maxFps", fps.to_string()),
        ("enableVsync", "false".into()),
        ("mipmapLevels", mipmap.to_string()),
        ("entityShadows", shadows.to_string()),
    ];
    if minor >= 16 {
        v.push(("graphicsMode", fancy.to_string()));
        v.push(("biomeBlendRadius", blend.to_string()));
        v.push(("entityDistanceScaling", entity_dist.into()));
    } else {
        v.push(("fancyGraphics", (fancy == 1).to_string()));
    }
    if minor >= 18 {
        v.push(("simulationDistance", sim.to_string()));
    }
    if minor >= 19 {
        v.push(("ao", ao.to_string()));
        v.push(("prioritizeChunkUpdates", "0".into()));
    } else {
        v.push(("ao", if ao { "2" } else { "0" }.into()));
    }
    if minor >= 13 {
        v.push(("renderClouds", format!("\"{clouds}\"")));
    } else {
        v.push(("renderClouds", clouds.to_string()));
    }
    v
}

/// Aplica o preset ao `options.txt` e, se o Sodium estiver instalado, às opções dele.
pub async fn apply_graphics_preset(game_dir: &Path, preset: GraphicsPreset, mc_version: &str) -> Result<()> {
    merge_options(game_dir, &preset_values(preset, mc_version)).await?;
    apply_sodium_preset(game_dir, preset).await
}

async fn apply_sodium_preset(game_dir: &Path, preset: GraphicsPreset) -> Result<()> {
    let path = game_dir.join("config/sodium-options.json");
    let mut root: serde_json::Value = match tokio::fs::read(&path).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    let fast = preset == GraphicsPreset::Potato;
    let q = if fast { "FAST" } else { "DEFAULT" };
    let obj = root.as_object_mut().ok_or_else(|| LauncherError::Other("sodium-options.json inválido".into()))?;
    let quality = obj.entry("quality").or_insert_with(|| serde_json::json!({}));
    if let Some(m) = quality.as_object_mut() {
        m.insert("weather_quality".into(), q.into());
        m.insert("leaves_quality".into(), q.into());
        m.insert("enable_vignette".into(), (!fast).into());
    }
    let perf = obj.entry("performance").or_insert_with(|| serde_json::json!({}));
    if let Some(m) = perf.as_object_mut() {
        m.insert("animate_only_visible_textures".into(), true.into());
        m.insert("use_entity_culling".into(), true.into());
        m.insert("use_fog_occlusion".into(), true.into());
        m.insert("use_block_face_culling".into(), true.into());
    }
    write_atomic(&path, &serde_json::to_vec_pretty(&root)?).await
}

// ---------------------------------------------------------------- JVM

/// Class Data Sharing dinâmico (JDK 19+): na 1ª execução a JVM grava as classes
/// carregadas num arquivo `.jsa`; nas seguintes, mapeia direto da memória, o que
/// corta segundos da abertura. Se mods mudarem, a JVM recria o arquivo sozinha.
pub fn fast_startup_flags(java_major: u32, instance_dir: &Path) -> Vec<String> {
    if java_major < 19 {
        return Vec::new();
    }
    let archive: PathBuf = instance_dir.join(".cache").join("aurora-cds.jsa");
    let _ = std::fs::create_dir_all(instance_dir.join(".cache"));
    vec![format!("-XX:SharedArchiveFile={}", archive.display()), "-XX:+AutoCreateSharedArchive".into()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_formats_by_version() {
        let modern = preset_values(GraphicsPreset::Potato, "1.21.1");
        assert!(modern.contains(&("ao", "false".into())));
        assert!(modern.contains(&("simulationDistance", "5".into())));
        let old = preset_values(GraphicsPreset::Potato, "1.8.9");
        assert!(old.contains(&("ao", "0".into())));
        assert!(old.contains(&("fancyGraphics", "false".into())));
        assert!(!old.iter().any(|(k, _)| *k == "simulationDistance"));
    }

    #[test]
    fn cds_only_on_new_java() {
        assert!(fast_startup_flags(17, Path::new("/tmp/x")).is_empty());
        assert_eq!(fast_startup_flags(21, &std::env::temp_dir()).len(), 2);
    }

    #[test]
    fn curated_keys_unique() {
        let mut keys = HashSet::new();
        for m in PERFORMANCE.iter().chain(PVP) {
            assert!(keys.insert(m.key), "chave duplicada: {}", m.key);
        }
    }
}
