//! Orquestração completa: da instância no disco até a JVM rodando.

pub mod args;
pub mod filelog;
pub mod process;

use std::sync::Arc;

use serde::Serialize;

use crate::auth::microsoft::now_secs;
use crate::download::assets::{materialize_legacy_assets, prepare_assets};
use crate::download::libraries::{client_jar_task, plan_libraries};
use crate::download::natives::extract_natives;
use crate::download::{Downloader, Progress, ProgressSink, Stage};
use crate::error::{LauncherError, Result};
use crate::instance::{load_instance, save_instance};
use crate::java::{ensure_java, probe_java_major, required_java};
use crate::loaders::ensure_loader_profile;
use crate::skins::csl::ensure_custom_skin_loader;
use crate::version::rules::Environment;
use crate::version::{fetch_manifest, resolve_version};
use crate::Launcher;
use args::{build_arguments, redact, ArgsInput};
use process::{spawn_game, SpawnSpec};
pub use process::{ExitInfo, GameProcess, LogLevel, LogLine, LogStream};

/// Recebe tudo que acontece durante a preparação e a execução.
pub trait LaunchObserver: ProgressSink {
    fn log(&self, line: LogLine);
    fn exited(&self, info: ExitInfo);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchStarted {
    pub instance_id: String,
    pub pid: Option<u32>,
    pub java: String,
    pub java_major: u32,
    pub version_id: String,
}

fn info(observer: &dyn LaunchObserver, instance_id: &str, level: LogLevel, msg: impl Into<String>) {
    observer.log(LogLine { instance_id: instance_id.to_owned(), stream: LogStream::Launcher, level, line: msg.into() });
}

fn stage(observer: &dyn LaunchObserver, stage: Stage) {
    observer.progress(Progress { stage, files_done: 0, files_total: 0, bytes_done: 0, bytes_total: 0, current: None });
}

pub async fn launch_instance(
    launcher: &Launcher,
    instance_id: &str,
    account_id: &str,
    observer: Arc<dyn LaunchObserver>,
) -> Result<(GameProcess, LaunchStarted)> {
    crate::instance::load_instance(&launcher.paths, instance_id).await?;
    let tee = filelog::FileTee::new(observer, &launcher.paths.instance_dir(instance_id));
    tee.write(&format!("Aurora Launcher {} — {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS));
    let result = launch_inner(launcher, instance_id, account_id, tee.clone()).await;
    if let Err(e) = &result {
        tee.write(&format!("[launcher] ERRO: {e}"));
        tee.exited_without_process();
    }
    result
}

async fn launch_inner(
    launcher: &Launcher,
    instance_id: &str,
    account_id: &str,
    observer: Arc<filelog::FileTee>,
) -> Result<(GameProcess, LaunchStarted)> {
    let log_path = observer.path.clone();
    let observer: Arc<dyn LaunchObserver> = observer;
    let obs: &dyn LaunchObserver = observer.as_ref();
    let paths = &launcher.paths;
    let settings = launcher.settings.read().await.clone();
    let client = &launcher.client;
    let downloader = Downloader::new(client.clone(), settings.download_concurrency);

    let mut instance = load_instance(paths, instance_id).await?;

    // 1) Autenticação primeiro: falhas de login aparecem antes de qualquer download
    let auth = launcher.accounts.launch_auth(client, settings.client_id().as_deref(), account_id).await?;
    info(
        obs,
        instance_id,
        LogLevel::Info,
        format!("Conta: {} ({})", auth.username, if auth.is_offline { "offline" } else { "Microsoft" }),
    );

    // 2) Versão + modloader
    stage(obs, Stage::Manifest);
    let manifest = match fetch_manifest(client, paths).await {
        Ok(m) => Some(m),
        Err(e) => {
            info(obs, instance_id, LogLevel::Warn, format!("Manifest indisponível, usando arquivos locais: {e}"));
            None
        }
    };
    stage(obs, Stage::Loader);
    let version_id = ensure_loader_profile(client, paths, &mut instance).await?;
    let resolved = resolve_version(client, paths, manifest.as_ref(), &version_id).await?;
    info(obs, instance_id, LogLevel::Info, format!("Versão resolvida: {}", resolved.chain.join(" → ")));

    // 3) Java
    stage(obs, Stage::Java);
    let java = match &instance.java_path {
        Some(custom) if custom.is_file() => custom.clone(),
        Some(custom) => return Err(LauncherError::Java(format!("Java configurado não existe: {}", custom.display()))),
        None => {
            let major = required_java(&resolved.json, &instance.minecraft_version);
            info(obs, instance_id, LogLevel::Info, format!("Java necessário: {major}"));
            ensure_java(paths, &downloader, major, obs).await?
        }
    };
    let java_major = probe_java_major(&java).await?;

    // 4) Client + libraries
    let env = Environment::current().with_feature("has_custom_resolution", instance.resolution.is_some());
    let libs = plan_libraries(paths, &resolved.json, &env);
    if let Some(missing) = libs.required_local.iter().find(|p| !p.exists()) {
        return Err(LauncherError::Loader(format!(
            "library gerada pelo instalador não encontrada: {} (reinstale o modloader)",
            missing.display()
        )));
    }
    let client_task = client_jar_task(paths, &resolved)
        .ok_or_else(|| LauncherError::VersionNotFound(format!("{} não define o client JAR", resolved.client_jar_id)))?;
    let client_jar = client_task.path.clone();
    downloader.download_one(Stage::Client, client_task, obs).await?;
    downloader.run(Stage::Libraries, libs.downloads.clone(), obs).await?;

    // 5) Assets
    let index_ref = resolved
        .json
        .asset_index
        .as_ref()
        .ok_or_else(|| LauncherError::VersionNotFound("versão sem assetIndex".into()))?;
    let mut assets = prepare_assets(paths, &downloader, index_ref, obs).await?;
    downloader.run(Stage::Assets, std::mem::take(&mut assets.tasks), obs).await?;

    let game_dir = paths.instance_game_dir(&instance.id);
    tokio::fs::create_dir_all(&game_dir).await?;
    let game_assets = materialize_legacy_assets(paths, &assets, &game_dir).await?;

    // 6) Cosméticos + skins para contas offline (não bloqueia o jogo se falhar)
    if let Some(account) = launcher.accounts.get(account_id) {
        for warning in crate::cosmetics::prepare_for_launch(
            client,
            paths,
            &account,
            &game_dir,
            settings.cosmetics_server.as_deref(),
            instance.loader == crate::instance::ModLoader::Fabric,
            &instance.minecraft_version,
        )
        .await
        {
            info(obs, instance_id, LogLevel::Warn, warning);
        }
    }
    if auth.is_offline && instance.skin_loader && instance.loader.supports_mods() {
        match ensure_custom_skin_loader(
            paths,
            &downloader,
            &instance,
            settings.custom_skin_api.as_deref(),
            settings.cosmetics_server.as_deref(),
            obs,
        )
        .await
        {
            Ok(true) => {
                info(obs, instance_id, LogLevel::Info, "CustomSkinLoader pronto (LocalSkin → API → Ely.by → Mojang)")
            }
            Ok(false) => {
                info(obs, instance_id, LogLevel::Warn, "CustomSkinLoader não tem build para esta versão/loader")
            }
            Err(e) => {
                info(obs, instance_id, LogLevel::Warn, format!("Não foi possível instalar o CustomSkinLoader: {e}"))
            }
        }
    }

    // 7) Nativos
    stage(obs, Stage::Natives);
    let natives_dir = extract_natives(paths, libs.natives.clone()).await?;

    // 8) Argumentos
    stage(obs, Stage::Launching);
    let mut classpath = libs.classpath.clone();
    classpath.push(client_jar);
    let libraries_dir = paths.libraries();
    let assets_root = paths.assets();
    let extra_jvm = if instance.experimental_cds {
        crate::optimize::fast_startup_flags(java_major, &paths.instance_dir(&instance.id))
    } else {
        Vec::new()
    };
    if !extra_jvm.is_empty() {
        info(
            obs,
            instance_id,
            LogLevel::Info,
            "AppCDS experimental ativo. Se o jogo fechar sozinho ao abrir, desligue em Otimização.",
        );
    }
    let (jvm, main_class, game) = build_arguments(&ArgsInput {
        version: &resolved.json,
        auth: &auth,
        instance: &instance,
        env: &env,
        game_dir: &game_dir,
        natives_dir: &natives_dir,
        libraries_dir: &libraries_dir,
        assets_root: &assets_root,
        game_assets: game_assets.as_deref(),
        classpath: &classpath,
        extra_jvm: &extra_jvm,
    });
    let mut all_args = jvm;
    all_args.push(main_class);
    all_args.extend(game);
    info(
        obs,
        instance_id,
        LogLevel::Debug,
        format!("{} {}", java.display(), redact(&all_args, &auth.access_token).join(" ")),
    );

    instance.last_played = Some(now_secs());
    save_instance(paths, &instance).await?;

    // 9) Execução
    let process = spawn_game(
        SpawnSpec {
            instance_id: instance.id.clone(),
            java: java.clone(),
            java_major,
            args: all_args,
            game_dir,
            natives_dir,
            attach_console: settings.keep_console_attached,
        },
        observer.clone(),
    )
    .await?;

    let started = LaunchStarted {
        instance_id: instance.id.clone(),
        pid: process.pid,
        java: java.display().to_string(),
        java_major,
        version_id,
    };
    info(obs, instance_id, LogLevel::Info, format!("Jogo iniciado (PID {:?}, Java {java_major})", process.pid));
    info(obs, instance_id, LogLevel::Info, format!("Log completo desta execução: {}", log_path.display()));
    Ok((process, started))
}
