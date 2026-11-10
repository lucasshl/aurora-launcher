//! Tipos do `version_manifest_v2.json` e do JSON de versão (vanilla, Fabric, Forge, NeoForge).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub time: String,
    pub release_time: String,
    pub sha1: String,
    #[serde(default)]
    pub compliance_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub main_class: Option<String>,
    /// Formato antigo (≤ 1.12.2): string única com os argumentos do jogo.
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    /// Formato novo (≥ 1.13): listas com regras.
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub java_version: Option<JavaVersionReq>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl ArgValue {
    pub fn values(&self) -> Vec<&str> {
        match self {
            ArgValue::One(s) => vec![s.as_str()],
            ArgValue::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default)]
    pub total_size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "map_to_resources")]
    pub map_to_resources: bool,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    /// Repositório Maven (usado por Fabric/Quilt, que não trazem `downloads`).
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    /// Formato antigo de nativos: `{ "windows": "natives-windows-${arch}" }`.
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionReq {
    #[serde(default)]
    pub component: Option<String>,
    pub major_version: u32,
}

/// Coordenada Maven `grupo:artefato:versão[:classificador][@extensão]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenCoord {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl MavenCoord {
    pub fn parse(name: &str) -> Option<Self> {
        let (coords, extension) = match name.split_once('@') {
            Some((c, e)) => (c, e.to_owned()),
            None => (name, "jar".to_owned()),
        };
        let parts: Vec<&str> = coords.split(':').collect();
        if parts.len() < 3 || parts.iter().any(|p| p.is_empty()) {
            return None;
        }
        Some(Self {
            group: parts[0].to_owned(),
            artifact: parts[1].to_owned(),
            version: parts[2].to_owned(),
            classifier: parts.get(3).map(|s| (*s).to_owned()),
            extension,
        })
    }

    /// Caminho relativo no repositório: `org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar`.
    pub fn path(&self) -> String {
        let file = match &self.classifier {
            Some(c) => format!("{}-{}-{}.{}", self.artifact, self.version, c, self.extension),
            None => format!("{}-{}.{}", self.artifact, self.version, self.extension),
        };
        format!("{}/{}/{}/{}", self.group.replace('.', "/"), self.artifact, self.version, file)
    }

    /// Chave para deduplicar libraries entre perfis herdados (ignora a versão).
    pub fn dedupe_key(&self) -> String {
        format!("{}:{}:{}", self.group, self.artifact, self.classifier.as_deref().unwrap_or(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_path() {
        let c = MavenCoord::parse("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap();
        assert_eq!(c.path(), "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar");
        let c = MavenCoord::parse("net.fabricmc:fabric-loader:0.16.9").unwrap();
        assert_eq!(c.path(), "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
        let c = MavenCoord::parse("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap();
        assert!(c.path().ends_with("mcp_config-1.20.1.zip"));
        assert!(MavenCoord::parse("invalido").is_none());
    }

    #[test]
    fn parses_conditional_arguments() {
        let json = r#"{"game":["--username","${auth_player_name}",
            {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],
             "value":["--width","${resolution_width}"]}],"jvm":[]}"#;
        let args: Arguments = serde_json::from_str(json).unwrap();
        assert_eq!(args.game.len(), 3);
        assert!(matches!(args.game[2], Argument::Conditional { .. }));
    }
}
