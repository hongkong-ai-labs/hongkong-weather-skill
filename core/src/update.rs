//! Self-update that reuses the method a binary was installed with.
//!
//! Supported methods:
//!
//! - `cargo install` (`~/.cargo/bin`): rerun `cargo install --git`, or
//!   `cargo install --path` when `<PREFIX>_SOURCE` points at a checkout.
//! - release installer (`~/.local/bin`): rerun `scripts/install.sh` from the
//!   checkout when present, otherwise download it from the repository.
//! - source build (`target/debug` or `target/release`): rebuild with cargo.
//! - Homebrew (`Cellar` or a Homebrew prefix): `brew upgrade`.
//! - container image: refuse, because the image must be rebuilt.
//!
//! Anything else is updated with the release installer only when the current
//! binary lives in a writable directory; otherwise the command explains how to
//! reinstall instead of writing somewhere it does not own.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Identity of the binary being updated.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// GitHub repository, `owner/name`.
    pub repo: &'static str,
    /// Binary name, as installed on `PATH`.
    pub binary: &'static str,
    /// Environment-variable prefix, for example `HKWEATHER`.
    pub env_prefix: &'static str,
    /// Version compiled into the binary.
    pub version: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Method {
    Cargo,
    Script,
    Source(PathBuf),
    Brew,
    Container,
    Unknown,
}

impl Method {
    pub fn label(&self) -> String {
        match self {
            Self::Cargo => "cargo install".to_string(),
            Self::Script => "release installer".to_string(),
            Self::Source(root) => format!("source build at {}", root.display()),
            Self::Brew => "Homebrew".to_string(),
            Self::Container => "container image".to_string(),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

/// Classify an executable path without touching the network or the filesystem
/// beyond `in_container`.
pub fn classify(exe: &Path, in_container: bool) -> Method {
    let path = exe.to_string_lossy().replace('\\', "/");
    if path.contains("/.cargo/bin/") {
        return Method::Cargo;
    }
    if in_container {
        return Method::Container;
    }
    if path.contains("/Cellar/") || path.contains("/homebrew/") || path.contains("/linuxbrew/") {
        return Method::Brew;
    }
    if let Some(root) = source_root(exe) {
        return Method::Source(root);
    }
    if path.contains("/.local/bin/") {
        return Method::Script;
    }
    Method::Unknown
}

fn source_root(exe: &Path) -> Option<PathBuf> {
    let profile = exe.parent()?;
    let name = profile.file_name()?;
    if name != "debug" && name != "release" {
        return None;
    }
    let target = profile.parent()?;
    if target.file_name()? != "target" {
        return None;
    }
    target.parent().map(Path::to_path_buf)
}

fn in_container() -> bool {
    Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
        || env::var_os("container").is_some()
}

/// Run `update` or `update --check` for `config`.
pub fn run(config: &Config, check: bool, force: bool) -> Result<()> {
    let exe = env::current_exe().context("cannot locate the running binary")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let method = classify(&exe, in_container());
    let current = config.version;
    let latest = latest_tag(config).ok();

    if check {
        println!("{} {current}", config.binary);
        println!("installed with: {}", method.label());
        match latest {
            Some(ref tag) => {
                println!("latest release: {tag}");
                if same_version(tag, current) {
                    println!("up to date");
                } else {
                    println!("run `{} update` to install {tag}", config.binary);
                }
            }
            None => println!(
                "latest release: unavailable (check the network or {}/releases)",
                config.repo
            ),
        }
        return Ok(());
    }

    if !force
        && let Some(ref tag) = latest
        && same_version(tag, current)
    {
        println!("{} {current} is up to date.", config.binary);
        return Ok(());
    }

    match method {
        Method::Cargo => update_with_cargo(config),
        Method::Script => update_with_installer(config, &exe),
        Method::Source(root) => update_from_source(config, &root),
        Method::Brew => update_with_brew(config),
        Method::Container => bail!(
            "this binary is running from a container image; rebuild or pull a newer image instead"
        ),
        Method::Unknown => {
            let dir = exe
                .parent()
                .context("the running binary has no parent directory")?;
            if !can_write(config, dir) {
                bail!(
                    "cannot update {}: it is not in a cargo, installer, source or Homebrew location, and {} is not writable. Reinstall with `sh scripts/install.sh` from the {} checkout.",
                    exe.display(),
                    dir.display(),
                    config.repo
                );
            }
            update_with_installer(config, &exe)
        }
    }
}

fn update_with_cargo(config: &Config) -> Result<()> {
    let mut command = Command::new("cargo");
    command.args(["install", "--locked", "--force"]);
    match env::var_os(format!("{}_SOURCE", config.env_prefix)) {
        Some(source) => {
            command.args(["--path", &source.to_string_lossy()]);
        }
        None => {
            command.args(["--git", &format!("https://github.com/{}", config.repo)]);
        }
    }
    let status = command.status().context("failed to run cargo")?;
    if !status.success() {
        bail!("cargo install exited with {status}");
    }
    println!("updated {} with cargo.", config.binary);
    Ok(())
}

fn update_with_installer(config: &Config, exe: &Path) -> Result<()> {
    let script = installer_script(config, exe)?;
    let bin_dir = exe
        .parent()
        .context("the running binary has no parent directory")?;
    let status = Command::new("sh")
        .arg(&script)
        .env(format!("{}_FORCE_INSTALL", config.env_prefix), "1")
        .env(format!("{}_BIN_DIR", config.env_prefix), bin_dir)
        .status()
        .context("failed to run the release installer")?;
    if script.starts_with(env::temp_dir()) {
        let _ = fs::remove_file(&script);
    }
    if !status.success() {
        bail!("the release installer exited with {status}");
    }
    println!("updated {} in {}.", config.binary, bin_dir.display());
    Ok(())
}

fn update_from_source(config: &Config, root: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .args(["build", "--release", "--locked"])
        .current_dir(root)
        .status()
        .context("failed to run cargo in the source checkout")?;
    if !status.success() {
        bail!("cargo build exited with {status}");
    }
    println!(
        "rebuilt the source checkout at {}; the new binary is {}",
        root.display(),
        root.join("target/release").join(config.binary).display()
    );
    Ok(())
}

fn update_with_brew(config: &Config) -> Result<()> {
    let status = Command::new("brew")
        .args(["upgrade", config.binary])
        .status()
        .context("failed to run brew")?;
    if !status.success() {
        bail!("brew upgrade {} exited with {status}", config.binary);
    }
    println!("updated {} with Homebrew.", config.binary);
    Ok(())
}

fn installer_script(config: &Config, exe: &Path) -> Result<PathBuf> {
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("scripts/install.sh");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    if let Method::Source(root) = classify(exe, false) {
        let candidate = root.join("scripts/install.sh");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    let url = format!(
        "https://raw.githubusercontent.com/{}/main/scripts/install.sh",
        config.repo
    );
    let path = env::temp_dir().join(format!(
        "{}-install-{}.sh",
        config.binary,
        std::process::id()
    ));
    fetch(&url, &path)?;
    Ok(path)
}

fn can_write(config: &Config, dir: &Path) -> bool {
    let probe = dir.join(format!(
        ".{}-write-test-{}",
        config.binary,
        std::process::id()
    ));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn latest_tag(config: &Config) -> Result<String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        config.repo
    );
    let text = fetch_text(&url)?;
    extract_tag(&text).context("the GitHub response had no release tag")
}

pub fn extract_tag(text: &str) -> Option<String> {
    let key = text.find("\"tag_name\"")?;
    let rest = &text[key + "\"tag_name\"".len()..];
    let rest = &rest[rest.find(':')? + 1..];
    let start = rest.find('"')? + 1;
    let end = rest[start..].find('"')?;
    Some(rest[start..start + end].to_string())
}

pub fn same_version(tag: &str, version: &str) -> bool {
    tag.trim_start_matches('v') == version.trim_start_matches('v')
}

fn fetch(url: &str, output: &Path) -> Result<()> {
    let curl = Command::new("curl")
        .args(["-fsSL", url, "-o"])
        .arg(output)
        .status();
    if let Ok(status) = curl
        && status.success()
    {
        return Ok(());
    }
    let status = Command::new("wget")
        .args(["-q", "-O"])
        .arg(output)
        .arg(url)
        .status()
        .context("need curl or wget to download the installer")?;
    if !status.success() {
        bail!("could not download {url}");
    }
    Ok(())
}

fn fetch_text(url: &str) -> Result<String> {
    let output = match Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", url])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => Command::new("wget")
            .args(["-q", "-O", "-", url])
            .output()
            .context("need curl or wget to check the latest release")?,
    };
    if !output.status.success() {
        bail!("could not fetch {url}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST: Config = Config {
        repo: "hongkong-ai-labs/hongkong-test-skill",
        binary: "hktest",
        env_prefix: "HKTEST",
        version: "0.4.2",
    };

    #[test]
    fn classifies_install_locations() {
        assert_eq!(
            classify(Path::new("/Users/me/.cargo/bin/hktest"), false),
            Method::Cargo
        );
        assert_eq!(
            classify(Path::new("/Users/me/.local/bin/hktest"), false),
            Method::Script
        );
        assert_eq!(
            classify(
                Path::new("/opt/homebrew/Cellar/hktest/1.0/bin/hktest"),
                false
            ),
            Method::Brew
        );
        assert_eq!(
            classify(Path::new("/usr/bin/hktest"), false),
            Method::Unknown
        );
        assert_eq!(
            classify(Path::new("/tmp/hktest/hktest"), true),
            Method::Container
        );
    }

    #[test]
    fn finds_the_source_root() {
        match classify(Path::new("/tmp/repo/target/release/hktest"), false) {
            Method::Source(root) => assert_eq!(root, Path::new("/tmp/repo")),
            other => panic!("expected a source build, got {other:?}"),
        }
    }

    #[test]
    fn parses_release_tags() {
        let body = r#"{"url":"https://api.github.com/repos/x/y/releases/1","tag_name": "v0.4.2","name":"v0.4.2"}"#;
        assert_eq!(extract_tag(body).as_deref(), Some("v0.4.2"));
        assert_eq!(extract_tag("{}"), None);
        assert!(same_version("v0.4.2", TEST.version));
        assert!(!same_version("v0.4.3", TEST.version));
    }
}
