use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

static LOGGER: LazyLock<Mutex<Option<LogFile>>> =
    LazyLock::new(|| Mutex::new(None));

struct LogFile {
    path: PathBuf,
}

impl LogFile {
    fn append(&self, msg: &str) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            writeln!(file, "{}", msg).ok();
        }
    }
}

fn timestamp() -> String {
    use time::OffsetDateTime;

    const FMT: &[time::format_description::BorrowedFormatItem] =
        time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

    OffsetDateTime::now_utc()
        .format(&FMT)
        .unwrap_or_else(|_| "????-??-?? ??:??:??".to_string())
}

pub fn init(path: PathBuf) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let mut guard = LOGGER.lock().expect("logger lock");
    *guard = Some(LogFile { path });
}

pub fn error(msg: &str) {
    let entry = format!("[{}] [ERROR] {}", timestamp(), msg);
    eprintln!("{}", entry);
    let guard = LOGGER.lock().expect("logger lock");
    if let Some(log) = guard.as_ref() {
        log.append(&entry);
    }
}

#[allow(dead_code)]
pub fn warn(msg: &str) {
    let entry = format!("[{}] [WARN] {}", timestamp(), msg);
    eprintln!("{}", entry);
    let guard = LOGGER.lock().expect("logger lock");
    if let Some(log) = guard.as_ref() {
        log.append(&entry);
    }
}

#[allow(dead_code)]
pub fn info(msg: &str) {
    let entry = format!("[{}] [INFO] {}", timestamp(), msg);
    let guard = LOGGER.lock().expect("logger lock");
    if let Some(log) = guard.as_ref() {
        log.append(&entry);
    }
}
