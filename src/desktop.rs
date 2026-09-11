use colored::Colorize;
use std::path::PathBuf;
use crate::TempestError;

const ICON_URL: &str = "https://pbs.twimg.com/profile_images/2060777230892933122/kQUQfyJt_400x400.jpg";

pub async fn create(game_id: u32) {
    println!("{}", "=== Desktop Entry ===".bold().cyan());

    let icon_dir = crate::config::Config::data_dir().join("icons");
    std::fs::create_dir_all(&icon_dir).ok();
    let icon_path = icon_dir.join("vortex.jpg");

    if icon_path.exists() {
        println!("{} Icon already downloaded.", "[PASS]".green());
    } else {
        println!("{} Downloading Vortex icon...", "[INFO]".cyan());
        match download_icon(&icon_path).await {
            Ok(()) => println!("{} Icon saved to {}", "[DONE]".green(), icon_path.display()),
            Err(e) => {
                eprintln!("{} Failed to download icon: {}", "[WARN]".yellow(), e);
                eprintln!("  Desktop entry will use a fallback icon.");
            }
        }
    }

    let app_dir = dirs::data_local_dir()
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".local/share")
        })
        .join("applications");
    std::fs::create_dir_all(&app_dir).ok();
    let desktop_path = app_dir.join("vortex.desktop");

    let exe = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("tempest"))
        .to_string_lossy()
        .into_owned();
    let exec = format!("{} play {}", exe, game_id);
    let icon_str = if icon_path.exists() {
        icon_path.to_string_lossy().into_owned()
    } else {
        "application-x-executable".to_string()
    };

    let content = format!(
        r#"[Desktop Entry]
Type=Application
Name=Vortex
Comment=Linux launcher for Vortex
Exec={exec}
Icon={icon}
Terminal=false
Categories=Game;
MimeType=x-scheme-handler/vortex;
StartupNotify=false
"#,
        exec = exec,
        icon = icon_str,
    );

    match std::fs::write(&desktop_path, &content) {
        Ok(()) => {
            println!("{} Created {}", "[DONE]".green(), desktop_path.display());
            let _ = std::process::Command::new("update-desktop-database")
                .arg(app_dir.to_string_lossy().to_string())
                .output();
            println!(
                "{} You can now launch Vortex from your application menu (game {}).",
                "[INFO]".cyan(),
                game_id
            );
        }
        Err(e) => {
            eprintln!("{} Failed to write desktop entry: {}", "[ERROR]".red(), e);
        }
    }
}

async fn download_icon(dest: &std::path::Path) -> Result<(), TempestError> {
    let client = reqwest::Client::builder()
        .user_agent(format!("tempest/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    let bytes = client.get(ICON_URL).send().await?.bytes().await?;
    std::fs::write(dest, &bytes)?;
    Ok(())
}