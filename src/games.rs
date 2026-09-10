use colored::Colorize;

const BASE: &str = "https://playvortex.io";

pub async fn list() {
    println!("{}", "=== Vortex Games ===".bold().cyan());

    let client = reqwest::Client::new();

    let resp = match client
        .get(format!("{BASE}/api/games"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} Failed to fetch games: {}", "[ERROR]".red(), e);
            return;
        }
    };

    let games: Vec<Game> = match resp.json().await {
        Ok(list) => list,
        Err(e) => {
            eprintln!("{} Failed to parse games: {}", "[ERROR]".red(), e);
            return;
        }
    };

    if games.is_empty() {
        println!("{} No games found.", "[INFO]".cyan());
        return;
    }

    println!(
        "{} Found {} game(s)\n",
        "[INFO]".cyan(),
        games.len().to_string().bold()
    );

    for game in &games {
        println!("  {:>5}  {}", game.id.to_string().bold(), game.name.bold());
    }

    println!();
    println!("  Run {} to launch a game.", "tempest play <id>".cyan());
}

#[derive(serde::Deserialize)]
pub struct Game {
    pub id: u32,
    pub name: String,
}
