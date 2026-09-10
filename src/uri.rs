use colored::Colorize;
use std::process::Command;
use url::Url;

pub fn build_uri(game_id: u32, token: &str) -> String {
    let mut url = Url::parse("vortex://play").expect("vortex scheme parses");
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("game", &game_id.to_string());
        pairs.append_pair("token", token);
    }
    url.to_string()
}

pub fn redact(uri: &str) -> String {
    match parse_vortex_uri(uri) {
        Some((game_id, _)) => format!("vortex://play?game={game_id}&token=<redacted>"),
        None => match uri.find("token=") {
            Some(idx) => {
                let mut s = uri[..idx + "token=".len()].to_string();
                s.push_str("<redacted>");
                s
            }
            None => "<invalid vortex:// uri>".to_string(),
        },
    }
}

pub fn parse_vortex_uri(uri: &str) -> Option<(u32, String)> {
    let parsed = Url::parse(uri).ok()?;
    if parsed.scheme() != "vortex" {
        return None;
    }
    let mut game_id = None;
    let mut token = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "game" => game_id = value.parse::<u32>().ok(),
            "token" => token = Some(value.into_owned()),
            _ => {}
        }
    }
    Some((game_id?, token?))
}

pub fn register() {
    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("/usr/local/bin/tempest"));

    let apps_dir = dirs::data_local_dir()
        .unwrap_or_default()
        .join("applications");
    std::fs::create_dir_all(&apps_dir).ok();

    let desktop_path = apps_dir.join("tempest-vortex.desktop");
    let contents = format!(
        "[Desktop Entry]\n\
         Name=Tempest (Vortex Launcher)\n\
         Exec={} uri-handler %u\n\
         Type=Application\n\
         MimeType=x-scheme-handler/vortex;\n\
         NoDisplay=true\n",
        exe_path.display()
    );

    if let Err(e) = std::fs::write(&desktop_path, &contents) {
        eprintln!("{} Failed to write .desktop file: {}", "[ERROR]".red(), e);
        return;
    }

    Command::new("xdg-mime")
        .args(["default", "tempest-vortex.desktop", "x-scheme-handler/vortex"])
        .status()
        .ok();

    Command::new("gio")
        .args(["mime", "x-scheme-handler/vortex", "tempest-vortex.desktop"])
        .status()
        .ok();

    Command::new("update-desktop-database")
        .arg(&apps_dir)
        .status()
        .ok();

    println!("{} Registered vortex:// URI handler", "[PASS]".green());
}

pub async fn handle(uri: &str) {
    tracing::debug!("Handling URI: {}", redact(uri));
    match parse_vortex_uri(uri) {
        Some((game_id, token)) => {
            println!("{} Launching game {} via URI", "[INFO]".cyan(), game_id);
            crate::launcher::play_with_token(game_id, token).await;
        }
        None => {
            eprintln!("{} Invalid vortex:// URI: {}", "[ERROR]".red(), uri);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_uri() {
        let result = parse_vortex_uri("vortex://play?game=4&token=abc123");
        assert_eq!(result, Some((4, "abc123".to_string())));
    }

    #[test]
    fn parse_invalid_scheme() {
        assert!(parse_vortex_uri("http://example.com").is_none());
    }

    #[test]
    fn parse_missing_token() {
        assert!(parse_vortex_uri("vortex://play?game=4").is_none());
    }

    #[test]
    fn build_uri_encodes_token() {
        let uri = build_uri(4, "abc+def/ghi=j&k#");
        assert_eq!(uri, "vortex://play?game=4&token=abc%2Bdef%2Fghi%3Dj%26k%23");
        let (id, token) = parse_vortex_uri(&uri).unwrap();
        assert_eq!(id, 4);
        assert_eq!(token, "abc+def/ghi=j&k#");
    }

    #[test]
    fn redact_masks_token() {
        let uri = build_uri(7, "secret");
        assert_eq!(redact(&uri), "vortex://play?game=7&token=<redacted>");
        assert!(!redact(&uri).contains("secret"));
    }
}
