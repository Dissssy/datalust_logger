use anyhow::Result;

pub(crate) fn parse_info(
    record: &log::Record,
    source: &str,
) -> Result<serde_json::Value> {
    if record.module_path().map(|s| s.contains("datalust_logger")).unwrap_or(false) {
        return Err(anyhow::anyhow!("Skipping log record from datalust_logger module"));
    }
    let now = chrono::Utc::now().to_rfc3339();
    let level = record.level();
    let mut log_entry = serde_json::json!({
        "@t": now,
        "@l": level.to_string(),
        "@mt": "[{source}::{target} | {level}] {msg}",
        "level": level.to_string(),
        "msg": record.args().to_string(),
        "source": source,
        "thread": std::thread::current().name().unwrap_or("main"),
        "target": record.target(),
    });
    if let Some(file) = record.file() {
        log_entry["file"] = file.into();
    }
    if let Some(line) = record.line() {
        log_entry["line"] = line.into();
    }
    if let Some(module_path) = record.module_path() {
        log_entry["module"] = module_path.into();
    }
    println!(
        "\x1b[36m[{}::{} | {}]:\x1b[0m {}",
        now,
        record.target(),
        level,
        record.args()
    );
    Ok(log_entry)
}

pub(crate) fn get_log_level() -> log::Level {
    match std::env::var("RUST_LOG") {
        Ok(level) => match level.as_str() {
            "error" => log::Level::Error,
            "warn" => log::Level::Warn,
            "info" => log::Level::Info,
            "debug" => log::Level::Debug,
            "trace" => log::Level::Trace,
            _ => log::Level::Info,
        },
        Err(_) => match std::env::var("SEQ_LOG_LEVEL") {
            Ok(level) => match level.as_str() {
                "error" => log::Level::Error,
                "warn" => log::Level::Warn,
                "info" => log::Level::Info,
                "debug" => log::Level::Debug,
                "trace" => log::Level::Trace,
                _ => log::Level::Info,
            },
            Err(_) => log::Level::Info,
        }
    }
}

pub(crate) fn get_api_url() -> Option<String> {
    match std::env::var("SEQ_API_URL") {
        Ok(url) => Some(url),
        Err(_) => None,
    }
}

pub(crate) fn get_api_key() -> Option<String> {
    match std::env::var("SEQ_API_KEY") {
        Ok(key) => Some(key),
        Err(_) => None,
    }
}