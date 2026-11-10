use std::collections::HashMap;

use super::model::{Rule, RuleAction};

/// Ambiente avaliado pelas regras (`rules`) de libraries e argumentos.
#[derive(Debug, Clone)]
pub struct Environment {
    /// `windows`, `osx` ou `linux` (nomes usados pela Mojang).
    pub os_name: &'static str,
    /// `std::env::consts::ARCH`: `x86_64`, `x86`, `aarch64`…
    pub arch: &'static str,
    pub features: HashMap<String, bool>,
}

impl Environment {
    pub fn current() -> Self {
        Self { os_name: os_name(), arch: std::env::consts::ARCH, features: HashMap::new() }
    }

    pub fn with_feature(mut self, name: &str, enabled: bool) -> Self {
        self.features.insert(name.to_owned(), enabled);
        self
    }

    pub fn is_64bit(&self) -> bool {
        matches!(self.arch, "x86_64" | "aarch64" | "powerpc64" | "riscv64")
    }
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn arch_matches(rule_arch: &str, env_arch: &str) -> bool {
    match rule_arch {
        "x86" => env_arch == "x86",
        "x86_64" | "x64" | "amd64" => env_arch == "x86_64",
        "arm64" | "aarch64" => env_arch == "aarch64",
        other => other == env_arch,
    }
}

impl Rule {
    pub fn matches(&self, env: &Environment) -> bool {
        if let Some(os) = &self.os {
            if let Some(name) = &os.name {
                if name != env.os_name {
                    return false;
                }
            }
            if let Some(arch) = &os.arch {
                if !arch_matches(arch, env.arch) {
                    return false;
                }
            }
            // `os.version` (regex de versão do SO) é usado só em casos antigos de macOS/Windows 10;
            // é tratado como compatível.
        }
        if let Some(features) = &self.features {
            for (name, expected) in features {
                if env.features.get(name).copied().unwrap_or(false) != *expected {
                    return false;
                }
            }
        }
        true
    }
}

/// Algoritmo oficial: sem regras = permitido; com regras, começa negado e a última regra que casar decide.
pub fn rules_allow(rules: &[Rule], env: &Environment) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule.matches(env) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::model::OsRule;

    fn env(os: &'static str) -> Environment {
        Environment { os_name: os, arch: "x86_64", features: HashMap::new() }
    }

    #[test]
    fn allow_all_but_osx() {
        let rules = vec![
            Rule { action: RuleAction::Allow, os: None, features: None },
            Rule {
                action: RuleAction::Disallow,
                os: Some(OsRule { name: Some("osx".into()), arch: None, version: None }),
                features: None,
            },
        ];
        assert!(rules_allow(&rules, &env("windows")));
        assert!(!rules_allow(&rules, &env("osx")));
    }

    #[test]
    fn features_default_false() {
        let mut f = HashMap::new();
        f.insert("is_demo_user".to_string(), true);
        let rules = vec![Rule { action: RuleAction::Allow, os: None, features: Some(f) }];
        assert!(!rules_allow(&rules, &env("linux")));
        assert!(rules_allow(&rules, &env("linux").with_feature("is_demo_user", true)));
    }
}
