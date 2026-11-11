//! Where `it` remembers hosts, tokens and the default repository.
//!
//! `~/.config/it/config.toml` (mode 0600) holds one entry per host. A `.it.toml`
//! in the working directory or any parent can name the repository (and host)
//! for a project, the way `gh` reads the git remote.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// The host used when none is given.
    pub default_host: Option<String>,
    #[serde(default)]
    pub hosts: BTreeMap<String, Host>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Host {
    pub token: String,
    pub user: String,
    /// `owner/name` used when no repository is given.
    pub default_repo: Option<String>,
}

/// `.it.toml`: per-directory settings.
#[derive(Debug, Default, Deserialize)]
pub struct Local {
    pub repo: Option<String>,
    pub host: Option<String>,
}

pub fn path() -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("IT_CONFIG_DIR") {
        return Ok(PathBuf::from(custom).join("config.toml"));
    }
    dirs::config_dir().map(|d| d.join("it").join("config.toml")).ok_or_else(|| "cannot find your config directory; set IT_CONFIG_DIR".to_string())
}

/// Hosts are stored without a trailing slash so one host has one entry.
pub fn normalize_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('/');
    if host.starts_with("http://") || host.starts_with("https://") {
        host.to_string()
    } else {
        format!("https://{host}")
    }
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let path = path()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{} is not valid: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = path()?;
        let dir = path.parent().ok_or("config path has no directory")?;
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_private(&path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }
}

/// The file holds tokens, so only the owner may read it.
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(text.as_bytes())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, text)
    }
}

/// The nearest `.it.toml`, looking from `start` upwards.
pub fn find_local(start: &Path) -> Local {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if let Ok(text) = std::fs::read_to_string(d.join(".it.toml")) {
            return toml::from_str(&text).unwrap_or_default();
        }
        dir = d.parent();
    }
    Local::default()
}

/// Which host to talk to: flag or `IT_HOST`, then `.it.toml`, then the saved default.
pub fn resolve_host(flag: Option<&str>, local: &Local, config: &Config) -> Result<String, String> {
    flag.map(normalize_host)
        .or_else(|| local.host.as_deref().map(normalize_host))
        .or_else(|| config.default_host.clone())
        .ok_or_else(|| "no host is set up. Run `it auth login --host https://your.server`".to_string())
}

/// Which repository a command is about: `-R` or `IT_REPO`, then `.it.toml`, then the host's default.
pub fn resolve_repo(flag: Option<&str>, local: &Local, host: Option<&Host>) -> Result<String, String> {
    let repo = flag
        .map(str::to_string)
        .or_else(|| local.repo.clone())
        .or_else(|| host.and_then(|h| h.default_repo.clone()))
        .ok_or_else(|| "no repository given. Use -R owner/name, or run `it repo set-default owner/name`".to_string())?;
    match repo.split_once('/') {
        Some((owner, name)) if !owner.is_empty() && !name.is_empty() && !name.contains('/') => Ok(repo),
        _ => Err(format!("\"{repo}\" is not a repository; expected owner/name")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_normalized() {
        assert_eq!(normalize_host("issues.example.com/"), "https://issues.example.com");
        assert_eq!(normalize_host(" http://127.0.0.1:23500 "), "http://127.0.0.1:23500");
    }

    #[test]
    fn repository_resolution_order() {
        let host = Host { token: "t".into(), user: "u".into(), default_repo: Some("me/default".into()) };
        let local = Local { repo: Some("me/local".into()), host: None };
        assert_eq!(resolve_repo(Some("a/b"), &local, Some(&host)).unwrap(), "a/b");
        assert_eq!(resolve_repo(None, &local, Some(&host)).unwrap(), "me/local");
        assert_eq!(resolve_repo(None, &Local::default(), Some(&host)).unwrap(), "me/default");
        assert!(resolve_repo(None, &Local::default(), None).is_err());
        for bad in ["nope", "a/", "/b", "a/b/c"] {
            assert!(resolve_repo(Some(bad), &Local::default(), None).is_err(), "{bad}");
        }
    }

    #[test]
    fn host_resolution_order() {
        let config = Config { default_host: Some("https://saved".into()), hosts: BTreeMap::new() };
        let local = Local { repo: None, host: Some("local.test".into()) };
        assert_eq!(resolve_host(Some("flag.test"), &local, &config).unwrap(), "https://flag.test");
        assert_eq!(resolve_host(None, &local, &config).unwrap(), "https://local.test");
        assert_eq!(resolve_host(None, &Local::default(), &config).unwrap(), "https://saved");
        assert!(resolve_host(None, &Local::default(), &Config::default()).is_err());
    }

    #[test]
    fn local_file_is_found_in_a_parent() {
        let root = std::env::temp_dir().join(format!("it-test-{}", std::process::id()));
        let deep = root.join("a").join("b");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(root.join(".it.toml"), "repo = \"me/app\"\n").unwrap();
        assert_eq!(find_local(&deep).repo.as_deref(), Some("me/app"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
