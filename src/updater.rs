use colored::Colorize;
use std::path::Path;
use crate::config::Config;
use crate::TempestError;

const DOWNLOAD_URL: &str = "https://playvortex.io/download/windows";

pub async fn update() {
    let cfg = Config::load();
    let dest = cfg.paths.vortex_exe.clone();

    println!("{} Downloading Vortex...", "[INFO]".cyan());

    match download_vortex(&dest, cfg.auth.session_token.as_deref()).await {
        Ok(()) => println!("{} Vortex.exe ready at {}", "[DONE]".green(), dest.display()),
        Err(e) => eprintln!("{} Update failed: {}", "[ERROR]".red(), e),
    }
}

async fn download_vortex(dest: &Path, session_token: Option<&str>) -> Result<(), TempestError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let client = reqwest::Client::new();

    let mut req = client.get(DOWNLOAD_URL);
    if let Some(token) = session_token {
        req = req.header("Cookie", format!("session_token={}", token));
    }

    println!("{} Downloading from {}", "[INFO]".cyan(), DOWNLOAD_URL);
    let resp = req.send().await?;

    if !resp.status().is_success() {
        return Err(TempestError::NetworkError(
            resp.error_for_status().unwrap_err(),
        ));
    }

    let total = resp.content_length();
    let pb = crate::setup::progress_bar(total);

    use futures_util::StreamExt;

    let zip_path = dest.with_extension("zip.tmp");
    let mut file = std::fs::File::create(&zip_path)?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(TempestError::NetworkError)?;
        std::io::Write::write_all(&mut file, &chunk)?;
        pb.inc(chunk.len() as u64);
    }
    pb.finish_with_message("Downloaded");

    println!("{} Extracting Vortex.exe from zip...", "[INFO]".cyan());
    let result = extract_exe_from_zip(&zip_path, dest);
    std::fs::remove_file(&zip_path).ok();
    result
}

fn extract_exe_from_zip(zip_path: &Path, dest: &Path) -> Result<(), TempestError> {
    use std::io::Write;

    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| TempestError::IoError(std::io::Error::other(e.to_string())))?;

    let exe_index = (0..archive.len()).find(|&i| {
        archive.by_index(i)
            .map(|f| {
                let name = f.name().to_lowercase();
                name.ends_with(".exe") && (name.contains("vortex") || name.contains("/vortex"))
            })
            .unwrap_or(false)
    });

    let index = exe_index.ok_or_else(|| {
        TempestError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No .exe found inside zip",
        ))
    })?;

    let mut file = archive.by_index(index)
        .map_err(|e| TempestError::IoError(std::io::Error::other(e.to_string())))?;

    println!("{} Found: {}", "[INFO]".cyan(), file.name());

    let tmp_path = dest.with_extension("exe.tmp");
    let mut out = std::fs::File::create(&tmp_path)?;
    std::io::copy(&mut file, &mut out)?;
    out.flush()?;
    drop(out);

    std::fs::rename(&tmp_path, dest)?;
    println!("{} Saved to {}", "[DONE]".green(), dest.display());

    Ok(())
}
