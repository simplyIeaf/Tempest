use colored::Colorize;
use crate::config::Config;
use crate::TempestError;

const BASE: &str = "https://playvortex.io";

enum LoginStep {
    Session(String),
    Needs2fa { pending_token: String },
}

pub async fn login() {
    println!("{}", "=== Vortex Login ===".bold().cyan());

    let username = prompt("Username: ");
    let password = prompt_hidden("Password: ");

    if username.is_empty() || password.is_empty() {
        eprintln!("{} Username and password required.", "[ERROR]".red());
        return;
    }

    println!("{} Signing in...", "[INFO]".cyan());
    match login_direct(&username, &password).await {
        Ok(LoginStep::Session(token)) => save_session(&username, token),

        Ok(LoginStep::Needs2fa { pending_token }) => {
            println!("{} Two-factor authentication required.", "[INFO]".cyan());
            let code = prompt_hidden("2FA or recovery code: ");
            if code.is_empty() {
                eprintln!("{} Code required.", "[ERROR]".red());
                return;
            }
            match login_2fa(&pending_token, &code).await {
                Ok(token) => save_session(&username, token),
                Err(e) => {
                    let msg = format!("2FA login failed: {}", e);
                    crate::logger::error(&msg);
                    eprintln!("{} {}", "[ERROR]".red(), msg);
                }
            }
        }

        Err(e) => {
            let msg = format!("Login failed: {}", e);
            crate::logger::error(&msg);
            eprintln!("{} {}", "[ERROR]".red(), msg);
        }
    }
}

fn save_session(username: &str, token: String) {
    let mut cfg = Config::load();
    cfg.auth.session_token = Some(token);
    cfg.auth.username = Some(username.to_string());
    if let Err(e) = cfg.save() {
        let msg = format!("Failed to save config: {}", e);
        crate::logger::error(&msg);
        eprintln!("{} {}", "[ERROR]".red(), msg);
    } else {
        println!("{} Logged in as {}", "[DONE]".green(), username.bold());
    }
}

fn login_client() -> Result<reqwest::Client, TempestError> {
    Ok(reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(format!("tempest/{}", env!("CARGO_PKG_VERSION")))
        .build()?)
}

fn session_token_from(resp: &reqwest::Response) -> Option<String> {
    resp.cookies()
        .filter(|c| c.name() == "session_token")
        .last()
        .map(|c| c.value().to_string())
}

async fn login_direct(username: &str, password: &str) -> Result<LoginStep, TempestError> {
    let client = login_client()?;

    let resp = client
        .post(format!("{BASE}/login"))
        .form(&[
            ("username", username),
            ("password", password),
            ("fingerprint", ""),
            ("fp_token", ""),
        ])
        .send()
        .await?;

    if resp.status().is_success() {
        if let Some(token) = session_token_from(&resp) {
            return Ok(LoginStep::Session(token));
        }
        if let Ok(body) = resp.json::<serde_json::Value>().await
            && body.get("requires_2fa").and_then(|v| v.as_bool()) == Some(true)
            && let Some(pending_token) = body.get("pending_token").and_then(|v| v.as_str())
        {
            return Ok(LoginStep::Needs2fa {
                pending_token: pending_token.to_string(),
            });
        }
        return Err(TempestError::AuthError(
            "server accepted login but set no session_token cookie".to_string(),
        ));
    }

    let body = resp.text().await.unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["detail"].as_str().map(str::to_string))
        .unwrap_or_else(|| "invalid username or password".to_string());
    Err(TempestError::AuthError(detail))
}

async fn login_2fa(pending_token: &str, code: &str) -> Result<String, TempestError> {
    let client = login_client()?;

    let resp = client
        .post(format!("{BASE}/login/2fa"))
        .form(&[
            ("code", code),
            ("pending_token", pending_token),
            ("fingerprint", ""),
            ("fp_token", ""),
        ])
        .send()
        .await?;

    let status = resp.status();
    let resp_cookie = resp
        .cookies()
        .filter(|c| c.name() == "session_token")
        .last()
        .map(|c| c.value().to_string());
    let body = resp.text().await.unwrap_or_default();
    if (status.is_success() || status.is_redirection()) && let Some(token) = resp_cookie {
        return Ok(token);
    }

    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["detail"].as_str().map(str::to_string))
        .unwrap_or_else(|| "invalid or expired code".to_string());
    Err(TempestError::AuthError(detail))
}

pub async fn get_play_uri(session_token: &str, game_id: u32) -> Result<String, TempestError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let resp = client
        .get(format!("{BASE}/games/{game_id}/play"))
        .header("Cookie", format!("session_token={session_token}"))
        .send()
        .await?;

    if resp.status().is_redirection() {
        return Err(TempestError::AuthError(format!(
            "Play page redirected (HTTP {}) - session may be expired. Run tempest login.",
            resp.status()
        )));
    }

    if !resp.status().is_success() {
        return Err(TempestError::AuthError(format!(
            "Failed to fetch play page: {}",
            resp.status()
        )));
    }

    let html = resp.text().await?;
    if let Some(start) = html.find("vortex://") {
        let uri = &html[start..];
        let end = uri
            .find(|c: char| c == '"' || c == '\'' || c.is_whitespace())
            .unwrap_or(uri.len());
        return Ok(uri[..end].to_string());
    }

    Err(TempestError::AuthError(
        "Could not find vortex:// URI in play page. Is the session token valid?".to_string(),
    ))
}

fn prompt(label: &str) -> String {
    use std::io::Write;
    print!("{} {}", ">>>".cyan(), label);
    std::io::stdout().flush().ok();
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).ok();
    buf.trim().to_string()
}

fn prompt_hidden(label: &str) -> String {
    use std::io::{BufRead, Write};
    print!("{} {}", ">>>".cyan(), label);
    std::io::stdout().flush().ok();

    let fd = 0;
    let mut term = unsafe { std::mem::zeroed::<libc::termios>() };
    let have_term = unsafe { libc::tcgetattr(fd, &mut term) } == 0;
    let restore = term;
    if have_term {
        term.c_lflag &= !libc::ECHO;
        unsafe { libc::tcsetattr(fd, libc::TCSANOW, &term) };
    }

    let mut buf = String::new();
    std::io::stdin().lock().read_line(&mut buf).ok();

    if have_term {
        unsafe { libc::tcsetattr(fd, libc::TCSANOW, &restore) };
        println!();
    }
    buf.trim().to_string()
}
