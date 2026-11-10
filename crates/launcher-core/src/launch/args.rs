//! Montagem da linha de comando da JVM: memória, GC, `arguments.jvm`, main class e `arguments.game`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::auth::LaunchAuth;
use crate::instance::InstanceConfig;
use crate::version::model::{Argument, VersionJson};
use crate::version::rules::{rules_allow, Environment};

pub const LAUNCHER_NAME: &str = "AuroraLauncher";

pub fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

pub struct ArgsInput<'a> {
    pub version: &'a VersionJson,
    pub auth: &'a LaunchAuth,
    pub instance: &'a InstanceConfig,
    pub env: &'a Environment,
    pub game_dir: &'a Path,
    pub natives_dir: &'a Path,
    pub libraries_dir: &'a Path,
    pub assets_root: &'a Path,
    /// Pasta de assets "virtual" para versões antigas; `None` usa `assets_root`.
    pub game_assets: Option<&'a Path>,
    pub classpath: &'a [PathBuf],
    /// Flags adicionais calculadas pelo launcher (ex.: AppCDS).
    pub extra_jvm: &'a [String],
}

/// Aikar's Flags (G1GC), ajustadas para heap grande quando `max_mb` ≥ 12 GB.
pub fn aikar_flags(max_mb: u32) -> Vec<String> {
    let large = max_mb >= 12 * 1024;
    let (new_size, max_new, region, reserve, ihop) =
        if large { (40, 50, "16M", 15, 20) } else { (30, 40, "8M", 20, 15) };
    [
        "-XX:+UseG1GC".to_owned(),
        "-XX:+ParallelRefProcEnabled".into(),
        "-XX:MaxGCPauseMillis=200".into(),
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:+DisableExplicitGC".into(),
        format!("-XX:G1NewSizePercent={new_size}"),
        format!("-XX:G1MaxNewSizePercent={max_new}"),
        format!("-XX:G1HeapRegionSize={region}"),
        format!("-XX:G1ReservePercent={reserve}"),
        "-XX:G1HeapWastePercent=5".into(),
        "-XX:G1MixedGCCountTarget=4".into(),
        format!("-XX:InitiatingHeapOccupancyPercent={ihop}"),
        "-XX:G1MixedGCLiveThresholdPercent=90".into(),
        "-XX:G1RSetUpdatingPauseTimePercent=5".into(),
        "-XX:SurvivorRatio=32".into(),
        "-XX:+PerfDisableSharedMem".into(),
        "-XX:MaxTenuringThreshold=1".into(),
    ]
    .into()
}

fn variables(input: &ArgsInput) -> HashMap<&'static str, String> {
    let p = |path: &Path| path.to_string_lossy().into_owned();
    let cp = input.classpath.iter().map(|c| p(c)).collect::<Vec<_>>().join(classpath_separator());
    let auth = input.auth;
    let assets_index = input
        .version
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| input.version.assets.clone())
        .unwrap_or_else(|| "legacy".into());
    let game_assets = input.game_assets.unwrap_or(input.assets_root);
    let (w, h) = input.instance.resolution.map(|r| (r.width, r.height)).unwrap_or((854, 480));

    HashMap::from([
        ("auth_player_name", auth.username.clone()),
        ("auth_uuid", auth.uuid_simple()),
        ("auth_access_token", auth.access_token.clone()),
        ("auth_session", format!("token:{}:{}", auth.access_token, auth.uuid_simple())),
        ("auth_xuid", auth.xuid.clone().unwrap_or_else(|| "0".into())),
        ("clientid", "0".into()),
        ("user_type", auth.user_type.clone()),
        ("user_properties", "{}".into()),
        ("version_name", input.version.id.clone()),
        ("version_type", input.version.kind.clone().unwrap_or_else(|| "release".into())),
        ("game_directory", p(input.game_dir)),
        ("assets_root", p(input.assets_root)),
        ("game_assets", p(game_assets)),
        ("assets_index_name", assets_index),
        ("natives_directory", p(input.natives_dir)),
        ("library_directory", p(input.libraries_dir)),
        ("classpath_separator", classpath_separator().into()),
        ("classpath", cp),
        ("launcher_name", LAUNCHER_NAME.into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("resolution_width", w.to_string()),
        ("resolution_height", h.to_string()),
    ])
}

/// Substitui `${chave}`. Retorna `None` se sobrar alguma variável desconhecida.
fn substitute(template: &str, vars: &HashMap<&'static str, String>) -> Option<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}')?;
        out.push_str(vars.get(&after[..end])?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

fn expand(args: &[Argument], env: &Environment) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(s.clone()),
            Argument::Conditional { rules, value } => {
                if rules_allow(rules, env) {
                    out.extend(value.values().into_iter().map(str::to_owned));
                }
            }
        }
    }
    out
}

/// Aplica substituições; quando um valor não pode ser resolvido, remove-o junto
/// com a flag `--opcao` que o precede (ex.: `--quickPlayPath ${quickPlayPath}`).
fn resolve_all(raw: Vec<String>, vars: &HashMap<&'static str, String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    for item in raw {
        match substitute(&item, vars) {
            Some(v) => out.push(v),
            None => {
                tracing::debug!("argumento sem valor descartado: {item}");
                if out.last().is_some_and(|prev| prev.starts_with("--")) && !item.starts_with('-') {
                    out.pop();
                }
            }
        }
    }
    out
}

/// Retorna `(jvm_args, main_class, game_args)`.
pub fn build_arguments(input: &ArgsInput) -> (Vec<String>, String, Vec<String>) {
    let vars = variables(input);
    let inst = input.instance;
    let max = inst.memory_max_mb.max(512);
    let min = inst.memory_min_mb.clamp(256, max);

    // --- JVM ---
    let mut jvm = vec![format!("-Xms{min}M"), format!("-Xmx{max}M")];
    if inst.optimized_gc {
        jvm.extend(aikar_flags(max));
    }
    jvm.push("-Dlog4j2.formatMsgNoLookups=true".into());
    jvm.extend(input.extra_jvm.iter().cloned());
    jvm.extend(inst.jvm_args.iter().filter(|a| !a.trim().is_empty()).cloned());

    let version_jvm = match input.version.arguments.as_ref().filter(|a| !a.jvm.is_empty()) {
        Some(a) => expand(&a.jvm, input.env),
        None => {
            // Versões ≤ 1.12.2 não trazem `arguments.jvm`: usa os padrões do launcher oficial
            let mut legacy = Vec::new();
            if cfg!(windows) {
                legacy.push(
                    "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into(),
                );
            }
            if cfg!(target_os = "macos") {
                legacy.push("-XstartOnFirstThread".into());
            }
            legacy.extend([
                "-Djava.library.path=${natives_directory}".to_owned(),
                "-Dminecraft.launcher.brand=${launcher_name}".into(),
                "-Dminecraft.launcher.version=${launcher_version}".into(),
                "-cp".into(),
                "${classpath}".into(),
            ]);
            legacy
        }
    };
    jvm.extend(resolve_all(version_jvm, &vars));

    // --- Main class ---
    let main_class = input.version.main_class.clone().unwrap_or_else(|| "net.minecraft.client.main.Main".into());

    // --- Jogo ---
    let raw_game = match (&input.version.arguments, &input.version.minecraft_arguments) {
        (Some(a), _) if !a.game.is_empty() => expand(&a.game, input.env),
        (_, Some(legacy)) => legacy.split_whitespace().map(str::to_owned).collect(),
        _ => Vec::new(),
    };
    let mut game = resolve_all(raw_game, &vars);
    if input.version.arguments.is_none() {
        if let Some(r) = inst.resolution {
            game.extend(["--width".into(), r.width.to_string(), "--height".into(), r.height.to_string()]);
        }
    }

    (jvm, main_class, game)
}

/// Remove o access token de uma linha de comando antes de exibir/logar.
pub fn redact(args: &[String], token: &str) -> Vec<String> {
    args.iter().map(|a| if token.len() > 4 { a.replace(token, "••••••") } else { a.clone() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> HashMap<&'static str, String> {
        HashMap::from([("auth_player_name", "Lucas".to_string()), ("version_name", "1.21.1".to_string())])
    }

    #[test]
    fn substitutes_known_vars() {
        assert_eq!(substitute("--username=${auth_player_name}", &vars()).unwrap(), "--username=Lucas");
        assert!(substitute("${quickPlayPath}", &vars()).is_none());
    }

    #[test]
    fn drops_flag_with_unknown_value() {
        let raw = vec![
            "--username".into(),
            "${auth_player_name}".into(),
            "--quickPlayPath".into(),
            "${quickPlayPath}".into(),
        ];
        assert_eq!(resolve_all(raw, &vars()), vec!["--username", "Lucas"]);
    }

    #[test]
    fn aikar_has_g1() {
        assert!(aikar_flags(4096).contains(&"-XX:+UseG1GC".to_string()));
        assert!(aikar_flags(16384).contains(&"-XX:G1HeapRegionSize=16M".to_string()));
    }
}
