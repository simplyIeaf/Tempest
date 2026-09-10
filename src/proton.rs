use colored::Colorize;
use sha2::{Digest, Sha512};
use std::io::{Read, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::config::Config;
use crate::TempestError;

fn install_dir() -> PathBuf {
    Config::data_dir().join("proton")
}

const MANAGED: &str = "GE-Proton";

#[derive(Debug, PartialEq, Eq)]
pub enum BackendStatus {
    Ready { version: String },
    NotConfigured,
    MissingUmu,
    MissingProton { requested: String },
}

impl BackendStatus {
    pub fn describe(&self) -> &'static str {
        match self {
            BackendStatus::Ready { .. } => "ready",
            BackendStatus::NotConfigured => "proton disabled in config",
            BackendStatus::MissingUmu => "umu-run not found (install umu-launcher)",
            BackendStatus::MissingProton { .. } => "GE-Proton not found (run: tempest proton)",
        }
    }
}

pub fn status(cfg: &Config) -> BackendStatus {
    if !cfg.proton.enabled {
        return BackendStatus::NotConfigured;
    }
    if umu_binary(cfg).is_none() {
        return BackendStatus::MissingUmu;
    }
    match resolve_proton(cfg) {
        Some(dir) => BackendStatus::Ready {
            version: version_of(&dir).unwrap_or_else(|| dir.display().to_string()),
        },
        None => BackendStatus::MissingProton {
            requested: cfg.proton.proton_path.clone(),
        },
    }
}

pub fn status_report(cfg: &Config) {
    println!("{}", "=== Proton ===".bold().cyan());
    match status(cfg) {
        BackendStatus::Ready { version } => println!("{} status: {}", "[PASS]".green(), version.bold()),
        BackendStatus::MissingUmu => {
            println!("{} umu-run not found.", "[FAIL]".red());
            print_umu_hint();
        }
        BackendStatus::MissingProton { .. } => {
            println!("{} GE-Proton not installed.", "[FAIL]".red());
            println!("  Run {} to download and extract the latest GE-Proton.", "tempest proton".cyan());
        }
        BackendStatus::NotConfigured => {
            println!("{} Proton disabled in config. Falling back to Wine.", "[INFO]".cyan());
            println!("  Set [proton] enabled = true and run {} to enable.", "tempest proton".cyan());
        }
    }
    println!(
        "{} config: enabled={} umu={} proton_path={} game_id={} store={}",
        "[INFO]".cyan(),
        cfg.proton.enabled,
        cfg.proton.umu,
        cfg.proton.proton_path,
        cfg.proton.game_id,
        cfg.proton.store,
    );
    if let Some(dir) = find_installed_proton() {
        println!("{} managed install: {}", "[INFO]".cyan(), dir.display());
    }
}

fn print_umu_hint() {
    println!("  Install umu-launcher with your package manager:");
    println!("    Fedora/Nobara: dnf install umu-launcher");
    println!("    Arch Linux:    pacman -S umu-launcher");
    println!("    openSUSE:      zypper install umu-launcher");
    println!("  or a flatpak/pip build from https://github.com/Open-Wine-Components/umu-launcher");
    println!("  GE-Proton downloads Steam's runtime automatically on first launch.");
}

fn umu_binary(cfg: &Config) -> Option<String> {
    let umu = cfg.proton.umu.trim();
    if umu.is_empty() {
        return None;
    }
    if umu.contains('/') {
        return Path::new(umu).is_file().then(|| umu.to_string());
    }
    which::which(umu).ok().map(|p| p.to_string_lossy().into_owned())
}

fn resolve_proton(cfg: &Config) -> Option<PathBuf> {
    let requested = cfg.proton.proton_path.trim();
    if requested.is_empty() {
        return None;
    }
    if requested == MANAGED {
        return find_installed_proton();
    }
    let p = PathBuf::from(requested);
    p.is_dir().then_some(p)
}

fn find_installed_proton() -> Option<PathBuf> {
    let dir = install_dir();
    if !dir.is_dir() {
        return None;
    }
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir()
                && entry.file_name().to_string_lossy().starts_with("GE-Proton")
                && let Ok(meta) = std::fs::metadata(&p)
                && let Ok(mtime) = meta.modified()
            {
                candidates.push((mtime, p));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    candidates.into_iter().next().map(|(_, p)| p)
}

fn version_of(dir: &Path) -> Option<String> {
    dir.file_name()?.to_str().map(str::to_string)
}

pub(crate) fn build_command(cfg: &Config, uri: &str) -> Option<Command> {
    if !cfg.proton.enabled {
        return None;
    }
    let umu = umu_binary(cfg)?;
    let proton = resolve_proton(cfg)?;

    let mut cmd = Command::new(umu);
    cmd.env("WINEPREFIX", &cfg.paths.wine_prefix);
    cmd.env("GAMEID", &cfg.proton.game_id);
    cmd.env("STORE", &cfg.proton.store);
    cmd.env("PROTONPATH", &proton);
    cmd.env("WGPU_BACKEND", "vulkan");

    if cfg.launcher.use_esync {
        cmd.env("WINEESYNC", "1");
    }
    if cfg.launcher.use_fsync {
        cmd.env("WINEFSYNC", "1");
    }
    if cfg.launcher.fsr > 0 {
        cmd.env("WINE_FULLSCREEN_FSR", "1");
        cmd.env("WINE_FULLSCREEN_FSR_STRENGTH", cfg.launcher.fsr.to_string());
    }

    crate::launcher::apply_common_env(&mut cmd, cfg);

    cmd.arg(&cfg.paths.vortex_exe);
    cmd.arg(uri);
    Some(cmd)
}

pub fn run_status() {
    let cfg = Config::load();
    status_report(&cfg);
}

fn ensure_umu(cfg: &Config) -> bool {
    if umu_binary(cfg).is_some() {
        println!("{} umu-run found.", "[PASS]".green());
        return true;
    }
    println!("{} umu-run not found.", "[WARN]".yellow());

    let distro = crate::setup::detect_distro();
    let command = match &distro {
        crate::setup::Distro::Fedora => Some("sudo dnf install umu-launcher".to_string()),
        crate::setup::Distro::Arch => Some("sudo pacman -S umu-launcher".to_string()),
        crate::setup::Distro::OpenSuse => Some("sudo zypper install umu-launcher".to_string()),
        _ => None,
    };

    match command {
        Some(cmd) => {
            if crate::setup::prompt_confirm(&format!("Install umu-launcher for {} now?", distro)) {
                crate::setup::run_cmd(&cmd);
            }
        }
        None => {
            print_umu_hint();
            println!("  For Debian/Ubuntu, grab the matching umu-launcher .deb from");
            println!("  https://github.com/Open-Wine-Components/umu-launcher/releases");
        }
    }

    umu_binary(cfg).is_some()
}

pub async fn install() {
    println!("{}", "=== Proton Install ===".bold().cyan());

    let mut cfg = Config::load();
    if !ensure_umu(&cfg) {
        println!("{} umu-launcher missing - continuing GE-Proton download anyway.", "[WARN]".yellow());
    }

    let client = reqwest::Client::builder()
        .user_agent(format!("tempest/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("build reqwest client");

    println!("{} Querying latest GE-Proton release...", "[INFO]".cyan());
    let (tag, asset_url, checksum_url) = match latest_release(&client).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{} Failed to resolve GE-Proton release: {}", "[ERROR]".red(), e);
            return;
        }
    };

    let out_dir = install_dir();
    std::fs::create_dir_all(&out_dir).ok();
    let archive_path = out_dir.join(format!("{tag}.tar.gz"));

    if archive_path.exists() {
        println!("{} {} already downloaded.", "[PASS]".green(), tag);
    } else if let Err(e) = download_archive(&client, &asset_url, &archive_path).await {
        eprintln!("{} Download failed: {}", "[ERROR]".red(), e);
        std::fs::remove_file(&archive_path).ok();
        return;
    }

    match checksum_url {
        Some(url) => match verify_checksum(&client, &url, &archive_path).await {
            Ok(true) => println!("{} Checksum verified.", "[PASS]".green()),
            Ok(false) => {
                eprintln!("{} Checksum mismatch - aborting. Try again later.", "[ERROR]".red());
                return;
            }
            Err(e) => println!(
                "{} Could not verify checksum (continuing): {}",
                "[WARN]".yellow(),
                e
            ),
        },
        None => println!("{} No checksum asset published; skipping verification.", "[WARN]".yellow()),
    }

    match extract_archive(&archive_path, &out_dir) {
        Ok(Some(version)) => println!("{} Extracted {}", "[DONE]".green(), version.bold()),
        Ok(None) => println!("{} Already extracted.", "[DONE]".green()),
        Err(e) => {
            eprintln!("{} Extraction failed: {}", "[ERROR]".red(), e);
            return;
        }
    }

    cfg.proton.enabled = true;
    cfg.proton.proton_path = MANAGED.to_string();
    if let Err(e) = cfg.save() {
        eprintln!("{} Could not persist config: {}", "[WARN]".yellow(), e);
    } else {
        println!("{} Proton enabled in config.", "[DONE]".green());
    }
}

async fn latest_release(
    client: &reqwest::Client,
) -> Result<(String, String, Option<String>), TempestError> {
    let api_url = "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/latest";
    let body: serde_json::Value = client
        .get(api_url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?
        .json()
        .await?;

    let tag = body["tag_name"]
        .as_str()
        .ok_or_else(|| TempestError::Other("no tag_name in release".into()))?
        .to_string();

    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        _ => "x86_64",
    };

    let assets = body["assets"]
        .as_array()
        .ok_or_else(|| TempestError::Other("no assets in release".into()))?;

    let mut archive: Option<(String, String)> = None;
    let mut generic_archive: Option<(String, String)> = None;
    let mut checksum: Option<String> = None;
    let mut generic_checksum: Option<String> = None;

    for a in assets {
        let name = a["name"].as_str().unwrap_or_default();
        let url = a["browser_download_url"].as_str().unwrap_or_default();
        if name.ends_with(".sha512sum") {
            if name.contains(arch) {
                checksum.get_or_insert_with(|| url.to_string());
            } else if !name.contains("aarch64") && generic_checksum.is_none() {
                generic_checksum = Some(url.to_string());
            }
        } else if name.ends_with(".tar.gz") {
            if name.contains(&format!("-{arch}")) && archive.is_none() {
                archive = Some((name.to_string(), url.to_string()));
            } else if arch == "x86_64"
                && archive.is_none()
                && !name.to_lowercase().contains("aarch64")
                && generic_archive.is_none()
            {
                generic_archive = Some((name.to_string(), url.to_string()));
            }
        }
    }

    let (name, url) = match archive.or(generic_archive) {
        Some(v) => v,
        None => return Err(TempestError::Other("no GE-Proton archive asset found".into())),
    };
    if url.is_empty() {
        return Err(TempestError::Other("archive asset has no download URL".into()));
    }
    let checksum = checksum.or(generic_checksum);

    tracing::debug!("selected asset {name}");
    Ok((tag, url, checksum))
}

async fn download_archive(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
) -> Result<(), TempestError> {
    use futures_util::StreamExt;

    let part = dest.with_extension("tar.gz.part");
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(TempestError::Other(format!("download failed: {}", resp.status())));
    }

    let pb = crate::setup::progress_bar(resp.content_length());
    let mut file = std::fs::File::create(&part)?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(TempestError::NetworkError)?;
        file.write_all(&chunk)?;
        pb.inc(chunk.len() as u64);
    }
    pb.finish_with_message("downloaded");
    drop(file);

    std::fs::rename(&part, dest)?;
    Ok(())
}

async fn verify_checksum(
    client: &reqwest::Client,
    url: &str,
    archive: &Path,
) -> Result<bool, TempestError> {
    let body = client.get(url).send().await?.text().await?;
    let expected = body
        .split_whitespace()
        .next()
        .ok_or_else(|| TempestError::Other("empty checksum file".into()))?
        .to_ascii_lowercase();

    let file = std::fs::File::open(archive)?;
    let mut hasher = Sha512::new();
    let mut reader = std::io::BufReader::new(file);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = format!("{:x}", hasher.finalize());
    Ok(actual == expected)
}

fn extract_archive(archive_path: &Path, out_dir: &Path) -> Result<Option<String>, TempestError> {
    let file = std::fs::File::open(archive_path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut ar = tar::Archive::new(gz);
    ar.set_overwrite(true);
    std::fs::create_dir_all(out_dir)?;
    ar.unpack(out_dir)?;

    let found = find_installed_proton();
    Ok(found.map(|p| version_of(&p).unwrap_or_else(|| "GE-Proton".to_string())))
}