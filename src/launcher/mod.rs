pub mod process;

use colored::Colorize;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use crate::config::Config;

fn perf_summary(config: &Config, use_gamemode: bool) -> String {
    let mut active: Vec<String> = vec![];
    if config.launcher.use_esync { active.push("esync".into()); }
    if config.launcher.use_fsync { active.push("fsync".into()); }
    if config.launcher.fsr > 0 { active.push(format!("fsr{}", config.launcher.fsr)); }
    if use_gamemode { active.push("gamemode".into()); }
    if config.launcher.shader_cache { active.push("shader-cache".into()); }
    if config.launcher.gpu_device != "auto" { active.push(format!("gpu={}", config.launcher.gpu_device)); }
    if !config.launcher.dxvk_hud.is_empty() { active.push("dxvk-hud".into()); }
    if config.launcher.keep_panel { active.push("panel".into()); }
    if is_wayland(config) { active.push("wayland".into()); }
    if active.is_empty() { "none".to_string() } else { active.join(" ") }
}

const NOISE_PATTERNS: &[&str] = &[
    "fixme:",
    "libEGL warning",
    "pci id for fd",
    "wine-staging",
    "experimental patches",
    "DxgiFactory::QueryInterface",
    "DxgiAdapter::QueryInterface",
    "create_factory_media",
    "EnableNonClientDpiScaling",
    "DwmSetWindowAttribute",
];

fn is_noise(line: &str) -> bool {
    NOISE_PATTERNS.iter().any(|p| line.contains(p))
}

fn build_launch_command(config: &Config, uri: &str, use_gamemode: bool) -> Result<Command, String> {
    let mut cmd = match crate::proton::build_command(config, uri) {
        Some(cmd) => cmd,
        None => build_wine_command(config, uri),
    };

    if use_gamemode {
        let mut gm = Command::new("gamemoderun");
        gm.arg(cmd.get_program());
        gm.args(cmd.get_args());
        for (key, value) in cmd.get_envs() {
            if let Some(v) = value {
                gm.env(key, v);
            }
        }
        gm.current_dir(cmd.get_current_dir().unwrap_or(std::path::Path::new(".")));
        cmd = gm;
    }
    Ok(cmd)
}

fn build_wine_command(config: &Config, uri: &str) -> Command {
    let perf = &config.launcher;

    let mut cmd = Command::new(&config.wine.binary);

    cmd.env("WINEPREFIX", &config.paths.wine_prefix);

    cmd.env("WGPU_BACKEND", "vulkan");

    if perf.use_esync { cmd.env("WINEESYNC", "1"); }
    if perf.use_fsync { cmd.env("WINEFSYNC", "1"); }

    apply_common_env(&mut cmd, config);

    cmd.arg(&config.paths.vortex_exe);
    cmd.arg(uri);
    cmd
}

pub(crate) fn apply_common_env(cmd: &mut Command, config: &Config) {
    let perf = &config.launcher;

    if perf.filter_wine_noise {
        cmd.env("WINEDEBUG", "-all");
    }

    if perf.shader_cache {
        let cache = dirs::cache_dir()
            .unwrap_or_else(|| {
                std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".cache")
            })
            .join("vortex-shaders");
        std::fs::create_dir_all(&cache).ok();
        cmd.env("VKD3D_SHADER_CACHE_PATH", &cache);

        let dxvk_cache = cache.join("dxvk");
        std::fs::create_dir_all(&dxvk_cache).ok();
        cmd.env("DXVK_STATE_CACHE_PATH", &dxvk_cache);

        let shader_cache = cache.join("gl");
        std::fs::create_dir_all(&shader_cache).ok();
        cmd.env("__GL_SHADER_DISK_CACHE", "1");
        cmd.env("__GL_SHADER_DISK_CACHE_PATH", &shader_cache);
        cmd.env("__GL_SHADER_DISK_CACHE_SKIP_CLEANUP", "1");

        let mesa_cache = cache.join("mesa");
        std::fs::create_dir_all(&mesa_cache).ok();
        cmd.env("MESA_SHADER_CACHE_DIR", &mesa_cache);
    }

    cmd.env("STAGING_SHARED_MEMORY", "1");
    cmd.env("WINE_LARGE_ADDRESS_AWARE", "1");
    cmd.env("__GLVND_DISALLOW_PATCHING", "1");

    if !perf.dxvk_hud.is_empty() {
        cmd.env("DXVK_HUD", &perf.dxvk_hud);
    }

    apply_gpu_env(cmd, config);
    apply_session_env(cmd, config);

    for (key, value) in &config.wine.env {
        cmd.env(key, value);
    }

    for (key, value) in crate::plugin::env_vars(config) {
        cmd.env(key, value);
    }
}

fn detect_session() -> String {
    std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase()
}

fn is_wayland(config: &Config) -> bool {
    match config.launcher.wayland_mode.as_str() {
        "on" => true,
        "off" => false,
        _ => detect_session() == "wayland" || std::env::var_os("WAYLAND_DISPLAY").is_some(),
    }
}

fn detect_nvidia_icd() -> Option<String> {
    let icd_dir = std::path::Path::new("/usr/share/vulkan/icd.d");
    for name in &["nvidia_icd.json", "nvidia_icd.x86_64.json", "nvidia_icd.i686.json"] {
        let path = icd_dir.join(name);
        if path.exists() {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    None
}

fn apply_gpu_env(cmd: &mut Command, config: &Config) {
    let mode = config.launcher.gpu_device.as_str();
    let nvidia_icd = detect_nvidia_icd();
    let has_nvidia = nvidia_icd.is_some();

    match mode {
        "nvidia" => {
            cmd.env("DXVK_FILTER_DEVICE_NAME", "NVIDIA");
            cmd.env("__NV_PRIME_RENDER_OFFLOAD", "1");
            cmd.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
            cmd.env("__GL_THREADED_OPTIMIZATIONS", "1");
            if let Some(ref icd) = nvidia_icd {
                cmd.env("VK_ICD_FILENAMES", icd);
            }
        }
        "intel" => {
            cmd.env("DXVK_FILTER_DEVICE_NAME", "Intel");
        }
        _ => {
            if has_nvidia {
                cmd.env("DXVK_FILTER_DEVICE_NAME", "NVIDIA");
                cmd.env("__NV_PRIME_RENDER_OFFLOAD", "1");
                cmd.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
                cmd.env("__GL_THREADED_OPTIMIZATIONS", "1");
            }
        }
    }
}

fn apply_session_env(cmd: &mut Command, config: &Config) {
    let wayland = is_wayland(config) && std::env::var_os("WAYLAND_DISPLAY").is_some();

    if wayland {
        cmd.env("PROTON_ENABLE_WAYLAND", "1");
        cmd.env("WAYLAND_DISPLAY", std::env::var("WAYLAND_DISPLAY").unwrap_or_default());
    } else {
        cmd.env("PROTON_ENABLE_WAYLAND", "0");
    }

    if config.launcher.keep_panel {
        cmd.env("WINE_DISABLE_FULLSCREEN_HACK", "1");
    }
}

pub async fn play(game_id: u32) {
    let mut cfg = Config::load();

    let token = match cfg.auth.session_token.clone() {
        Some(t) => t,
        None => {
            println!("{} Not logged in. Running login flow...", "[WARN]".yellow());
            crate::auth::login().await;
            cfg = Config::load();
            match cfg.auth.session_token.clone() {
                Some(t) => t,
                None => {
                    eprintln!("{} Login failed. Aborting.", "[ERROR]".red());
                    return;
                }
            }
        }
    };

    println!("{} Fetching play URI for game {}...", "[INFO]".cyan(), game_id);
    match crate::auth::get_play_uri(&token, game_id).await {
        Ok(uri) => launch_with_uri(uri).await,
        Err(e) => {
            eprintln!("{} Failed to get play URI: {}", "[ERROR]".red(), e);
            eprintln!("  Try {} to re-authenticate.", "tempest login".cyan());
        }
    }
}

pub async fn play_with_token(game_id: u32, token: String) {
    let uri = crate::uri::build_uri(game_id, &token);
    launch_with_uri(uri).await;
}

async fn launch_with_uri(uri: String) {
    let cfg = Config::load();

    if !cfg.paths.vortex_exe.exists() {
        eprintln!("{} Vortex.exe not found at {}", "[ERROR]".red(), cfg.paths.vortex_exe.display());
        eprintln!("  Run {} first.", "tempest setup".cyan());
        return;
    }

    let game_id = crate::uri::parse_vortex_uri(&uri)
        .map(|(id, _)| id.to_string())
        .unwrap_or_else(|| "?".to_string());

    let backend = if cfg.proton.enabled {
        match crate::proton::status(&cfg) {
            crate::proton::BackendStatus::Ready { .. } => "GE-Proton (umu)",
            other => {
                eprintln!(
                    "{} Proton requested but not ready: {}. Falling back to Wine.",
                    "[WARN]".yellow(),
                    other.describe()
                );
                "wine"
            }
        }
    } else {
        "wine"
    };

    let use_gamemode = cfg.launcher.use_gamemode && which::which("gamemoderun").is_ok();
    println!("{} Launching Vortex for game {} [{}] (backend: {})...",
        "[INFO]".cyan(), game_id, perf_summary(&cfg, use_gamemode).bold(), backend);
    tracing::debug!("Launch URI (token redacted): {}", crate::uri::redact(&uri));

    let mut pm = process::ProcessManager::new();
    pm.ensure_receiver(&cfg);

    let mut cmd = match build_launch_command(&cfg, &uri, use_gamemode) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} Failed to build launch command: {}", "[ERROR]".red(), e);
            return;
        }
    };
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} Failed to launch: {}", "[ERROR]".red(), e);
            eprintln!("  Is Wine/Proton installed? Try {} for diagnostics.", "tempest doctor".cyan());
            return;
        }
    };

    let child_id = child.id();
    ctrlc::set_handler(move || {
        unsafe { libc::kill(child_id as i32, libc::SIGTERM); }
    }).ok();

    let stderr = child.stderr.take().map(BufReader::new);
    let stdout = child.stdout.take().map(BufReader::new);
    let filter = cfg.launcher.filter_wine_noise;

    let stderr_handle = std::thread::spawn(move || {
        if let Some(reader) = stderr {
            for line in reader.lines().map_while(Result::ok) {
                if filter && is_noise(&line) { continue; }
                eprintln!("{}", line);
            }
        }
    });

    let stdout_handle = std::thread::spawn(move || {
        if let Some(reader) = stdout {
            for line in reader.lines().map_while(Result::ok) {
                if filter && is_noise(&line) { continue; }
                println!("{}", line);
            }
        }
    });

    let optim_handle = if crate::plugin::installed("vortex-optim") {
        crate::plugin::binary_path("vortex-optim").map(|path| {
            println!("{} Starting vortex-optim...", "[INFO]".cyan());
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                match std::process::Command::new(&path).spawn() {
                    Ok(mut c) => {
                        tracing::debug!("vortex-optim PID {}", c.id());
                        let _ = c.wait();
                    }
                    Err(e) => tracing::warn!("vortex-optim failed: {}", e),
                }
            })
        })
    } else {
        None
    };

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} Failed waiting for child: {}", "[ERROR]".red(), e);
            stderr_handle.join().ok();
            stdout_handle.join().ok();
            return;
        }
    };
    stderr_handle.join().ok();
    stdout_handle.join().ok();
    if let Some(h) = optim_handle { h.join().ok(); }

    drop(pm);

    match status.code() {
        Some(0) => println!("{} Game exited cleanly.", "[DONE]".green()),
        Some(code) => {
            eprintln!("{} Game exited with code {}.", "[WARN]".yellow(), code);
            eprintln!("  Run {} for diagnostics.", "tempest doctor".cyan());
        }
        None => {
            eprintln!("{} Process was terminated by a signal.", "[WARN]".yellow());
            eprintln!("  This may indicate a crash. Check Wine/Proton compatibility.");
        }
    }
}
