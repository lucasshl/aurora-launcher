//! Testes de integração sem internet: servidor HTTP local para o downloader e
//! um JSON de versão sintético cobrindo os formatos vanilla antigo/novo e Fabric.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use launcher_core::auth::LaunchAuth;
use launcher_core::download::hash::sha1_bytes;
use launcher_core::download::libraries::plan_libraries;
use launcher_core::download::{Checksum, DownloadTask, Downloader, NoProgress, Stage};
use launcher_core::instance::{create_instance, list_instances, ModLoader, NewInstance};
use launcher_core::launch::args::{build_arguments, classpath_separator, ArgsInput};
use launcher_core::paths::LauncherPaths;
use launcher_core::version::model::VersionJson;
use launcher_core::version::rules::Environment;
use launcher_core::LauncherError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn tmp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aurora-test-{name}-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Servidor HTTP mínimo. `/ok` devolve o corpo; `/flaky` falha (500) na 1ª requisição.
async fn serve(body: &'static [u8]) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { break };
            let n = counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let len = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..len]);
                let resp = if req.starts_with("GET /flaky") && n == 0 {
                    b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_vec()
                } else {
                    let mut r =
                        format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len())
                            .into_bytes();
                    r.extend_from_slice(body);
                    r
                };
                let _ = sock.write_all(&resp).await;
            });
        }
    });
    (format!("http://{addr}"), hits)
}

#[tokio::test]
async fn downloader_verifies_sha1_and_retries() {
    const BODY: &[u8] = b"conteudo-de-teste-do-launcher";
    let (base, hits) = serve(BODY).await;
    let root = tmp_root("dl");
    let dl = Downloader::new(reqwest::Client::new(), 4);

    // 1) Hash correto + servidor instável na primeira tentativa → retry e sucesso
    let good = root.join("a/good.bin");
    dl.run(
        Stage::Libraries,
        vec![DownloadTask::new(
            format!("{base}/flaky"),
            good.clone(),
            Checksum::sha1(Some(&sha1_bytes(BODY))),
            Some(BODY.len() as u64),
        )],
        &NoProgress,
    )
    .await
    .expect("download com retry deveria funcionar");
    assert_eq!(std::fs::read(&good).unwrap(), BODY);
    assert!(hits.load(Ordering::SeqCst) >= 2, "deveria ter tentado de novo após o 500");

    // 2) Arquivo já válido não é baixado de novo
    let before = hits.load(Ordering::SeqCst);
    dl.run(
        Stage::Libraries,
        vec![DownloadTask::new(format!("{base}/ok"), good.clone(), Checksum::sha1(Some(&sha1_bytes(BODY))), None)],
        &NoProgress,
    )
    .await
    .unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), before);

    // 3) Hash errado (arquivo corrompido) → erro, sem .part nem arquivo final
    let bad = root.join("b/bad.bin");
    let err = dl
        .run(
            Stage::Libraries,
            vec![DownloadTask::new(
                format!("{base}/ok"),
                bad.clone(),
                Checksum::sha1(Some("0000000000000000000000000000000000000000")),
                None,
            )],
            &NoProgress,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, LauncherError::HashMismatch { .. }), "erro inesperado: {err}");
    assert!(!bad.exists());
    assert!(!root.join("b/bad.bin.part").exists());

    // 4) Arquivo local corrompido é detectado e substituído
    std::fs::write(&good, b"lixo").unwrap();
    dl.run(
        Stage::Libraries,
        vec![DownloadTask::new(format!("{base}/ok"), good.clone(), Checksum::sha1(Some(&sha1_bytes(BODY))), None)],
        &NoProgress,
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&good).unwrap(), BODY);

    std::fs::remove_dir_all(root).ok();
}

const VERSION_JSON: &str = r#"{
  "id": "fabric-loader-0.16.9-1.20.1",
  "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
  "type": "release",
  "assetIndex": { "id": "5", "sha1": "x", "size": 1, "totalSize": 1, "url": "https://x" },
  "arguments": {
    "game": ["--username", "${auth_player_name}", "--uuid", "${auth_uuid}", "--accessToken", "${auth_access_token}",
             "--gameDir", "${game_directory}", "--assetIndex", "${assets_index_name}",
             {"rules":[{"action":"allow","features":{"is_quick_play_singleplayer":true}}],"value":["--quickPlaySingleplayer","${quickPlaySingleplayer}"]},
             "--quickPlayPath", "${quickPlayPath}"],
    "jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}",
            {"rules":[{"action":"allow","os":{"name":"osx"}}],"value":["-XstartOnFirstThread"]}]
  },
  "libraries": [
    { "name": "net.fabricmc:fabric-loader:0.16.9", "url": "https://maven.fabricmc.net/" },
    { "name": "org.lwjgl:lwjgl:3.3.1", "downloads": { "artifact": {
        "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar", "sha1": "abc", "size": 10, "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar" } } },
    { "name": "org.lwjgl:lwjgl:3.3.1:natives-linux", "rules": [{"action":"allow","os":{"name":"linux"}}],
      "downloads": { "artifact": { "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar", "sha1": "def", "size": 10,
        "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar" } } },
    { "name": "org.lwjgl:lwjgl:3.3.1:natives-windows", "rules": [{"action":"allow","os":{"name":"windows"}}],
      "downloads": { "artifact": { "path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar", "sha1": "ghi", "size": 10,
        "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar" } } },
    { "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4", "natives": { "linux": "natives-linux", "windows": "natives-windows-${arch}" },
      "extract": { "exclude": ["META-INF/"] },
      "downloads": { "classifiers": {
        "natives-linux": { "path": "old/natives-linux.jar", "sha1": "j", "size": 1, "url": "https://x/old-linux.jar" },
        "natives-windows-64": { "path": "old/natives-windows-64.jar", "sha1": "k", "size": 1, "url": "https://x/old-win64.jar" } } } }
  ]
}"#;

fn linux() -> Environment {
    Environment { os_name: "linux", arch: "x86_64", features: Default::default() }
}

#[test]
fn plans_libraries_for_all_formats() {
    let paths = LauncherPaths::new("/data");
    let version: VersionJson = serde_json::from_str(VERSION_JSON).unwrap();
    let plan = plan_libraries(&paths, &version, &linux());

    let urls: Vec<&str> = plan.downloads.iter().map(|t| t.url.as_str()).collect();
    assert!(urls.contains(&"https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"));
    assert!(urls.iter().any(|u| u.ends_with("natives-linux.jar")));
    assert!(!urls.iter().any(|u| u.contains("windows")), "nativos de Windows não devem entrar no Linux");
    assert!(urls.contains(&"https://x/old-linux.jar"));

    // 2 nativos: LWJGL 3 (classificador) + LWJGL 2 (formato antigo)
    assert_eq!(plan.natives.len(), 2);
    // Classpath: fabric-loader, lwjgl, lwjgl natives-linux (o jar antigo de nativos não entra)
    assert_eq!(plan.classpath.len(), 3);

    let win = Environment { os_name: "windows", arch: "x86_64", features: Default::default() };
    let plan_win = plan_libraries(&paths, &version, &win);
    assert!(plan_win.downloads.iter().any(|t| t.url == "https://x/old-win64.jar"), "${{arch}} deve virar 64");
}

#[test]
fn builds_full_command_line() {
    let root = tmp_root("args");
    let paths = LauncherPaths::new(&root);
    let version: VersionJson = serde_json::from_str(VERSION_JSON).unwrap();
    let auth = LaunchAuth {
        username: "Lucas".into(),
        uuid: "b50ad385-829d-3141-a216-7e7d7539ba7f".into(),
        access_token: "tok123".into(),
        user_type: "legacy".into(),
        xuid: None,
        is_offline: true,
    };
    let instance: launcher_core::instance::InstanceConfig = serde_json::from_value(serde_json::json!({
        "id": "teste", "name": "Teste", "minecraftVersion": "1.20.1", "loader": "fabric",
        "memoryMinMb": 1024, "memoryMaxMb": 4096
    }))
    .unwrap();
    let game_dir = paths.instance_game_dir("teste");
    let natives = root.join("natives/x");
    let cp = vec![root.join("a.jar"), root.join("b.jar")];
    let (jvm, main, game) = build_arguments(&ArgsInput {
        version: &version,
        auth: &auth,
        instance: &instance,
        env: &linux(),
        game_dir: &game_dir,
        natives_dir: &natives,
        libraries_dir: &paths.libraries(),
        assets_root: &paths.assets(),
        game_assets: None,
        classpath: &cp,
        extra_jvm: &[],
    });

    assert_eq!(jvm[0], "-Xms1024M");
    assert_eq!(jvm[1], "-Xmx4096M");
    assert!(jvm.contains(&"-XX:+UseG1GC".to_string()), "optimizedGc padrão = true");
    assert!(!jvm.contains(&"-XstartOnFirstThread".to_string()));
    let cp_idx = jvm.iter().position(|a| a == "-cp").unwrap();
    assert_eq!(jvm[cp_idx + 1], format!("{}{}{}", cp[0].display(), classpath_separator(), cp[1].display()));
    assert_eq!(main, "net.fabricmc.loader.impl.launch.knot.KnotClient");

    let joined = game.join(" ");
    assert!(joined.contains("--username Lucas"));
    assert!(joined.contains("--uuid b50ad385829d3141a2167e7d7539ba7f"), "UUID sem hífens");
    assert!(joined.contains("--assetIndex 5"));
    assert!(!joined.contains("quickPlay"), "flags sem valor devem ser removidas: {joined}");
    assert!(!joined.contains("${"));
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn instances_are_isolated() {
    let root = tmp_root("inst");
    let paths = LauncherPaths::new(&root);
    paths.ensure_layout().await.unwrap();
    let req = || NewInstance {
        name: "Meu Pack".into(),
        minecraft_version: "1.20.1".into(),
        loader: ModLoader::Fabric,
        loader_version: None,
        memory_max_mb: Some(6144),
    };
    let a = create_instance(&paths, req(), 1024, 4096).await.unwrap();
    let b = create_instance(&paths, req(), 1024, 4096).await.unwrap();
    assert_ne!(a.id, b.id, "nomes iguais geram ids distintos");
    for sub in ["mods", "saves", "config", "resourcepacks"] {
        assert!(paths.instance_game_dir(&a.id).join(sub).is_dir());
    }
    assert_eq!(list_instances(&paths).await.unwrap().len(), 2);
    assert_eq!(a.memory_max_mb, 6144);
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn mod_manager_reads_metadata_and_toggles() {
    use launcher_core::content::{list_installed, remove, set_enabled};
    use launcher_core::modrinth::ContentKind;
    use std::io::Write;

    let root = tmp_root("mods");
    let paths = LauncherPaths::new(&root);
    let mods = paths.instance_game_dir("pack").join("mods");
    std::fs::create_dir_all(&mods).unwrap();

    // JAR Fabric sintético
    let jar = std::fs::File::create(mods.join("sodium-fabric-0.6.0.jar")).unwrap();
    let mut zip = zip::ZipWriter::new(jar);
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("fabric.mod.json", opts).unwrap();
    zip.write_all(br#"{"id":"sodium","name":"Sodium","version":"0.6.0","description":"Rendering","authors":["JellySquid",{"name":"IMS"}]}"#).unwrap();
    zip.finish().unwrap();

    // JAR NeoForge sintético com versão vinda do MANIFEST
    let jar = std::fs::File::create(mods.join("jei.jar")).unwrap();
    let mut zip = zip::ZipWriter::new(jar);
    zip.start_file("META-INF/neoforge.mods.toml", opts).unwrap();
    zip.write_all(
        b"modLoader=\"javafml\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"JEI\"\n",
    )
    .unwrap();
    zip.start_file("META-INF/MANIFEST.MF", opts).unwrap();
    zip.write_all(b"Manifest-Version: 1.0\nImplementation-Version: 19.21.0\n").unwrap();
    zip.finish().unwrap();

    std::fs::write(mods.join("notas.txt"), "ignorar").unwrap();

    let list = list_installed(&paths, "pack", ContentKind::Mod).await.unwrap();
    assert_eq!(list.len(), 2, "arquivos que não são .jar são ignorados");
    let jei = list.iter().find(|m| m.file_name == "jei.jar").unwrap();
    assert_eq!(jei.meta.version.as_deref(), Some("19.21.0"));
    let sodium = list.iter().find(|m| m.file_name.starts_with("sodium")).unwrap();
    assert_eq!(sodium.meta.name.as_deref(), Some("Sodium"));
    assert_eq!(sodium.meta.authors, vec!["JellySquid", "IMS"]);

    set_enabled(&paths, "pack", ContentKind::Mod, "jei.jar", false).await.unwrap();
    assert!(mods.join("jei.jar.disabled").exists());
    let list = list_installed(&paths, "pack", ContentKind::Mod).await.unwrap();
    assert!(!list.iter().find(|m| m.file_name == "jei.jar").unwrap().enabled);

    assert!(
        set_enabled(&paths, "pack", ContentKind::Mod, "../fora.jar", true).await.is_err(),
        "path traversal bloqueado"
    );
    remove(&paths, "pack", ContentKind::Mod, "jei.jar").await.unwrap();
    assert!(!mods.join("jei.jar.disabled").exists());
    std::fs::remove_dir_all(root).ok();
}
