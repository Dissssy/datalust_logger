#![allow(static_mut_refs)]
use anyhow::Result;
use log::{Level, Metadata, Record};
use std::mem::MaybeUninit;
use std::sync::mpsc::{channel, Sender};

// pub fn init_local(
//     source: &str,
//     api_key: &str,
//     log_level: Level,
// ) -> Result<()> {
//     init(source, None, api_key, log_level)
// }

// pub fn init_remote(
//     source: &str,
//     url: &str,
//     api_key: &str,
//     log_level: Level,
// ) -> Result<()> {
//     init(source, Some(url), api_key, log_level)
// }

// #[macro_export]
// macro_rules! set_panic_handler {
//     ($source:expr) => {
//         std::panic::set_hook(Box::new(move |info| {
//             let msg = match info.payload().downcast_ref::<&str>() {
//                 Some(s) => s.to_string(),
//                 None => format!("{:?}", info),
//             };
//             log::error!("{}", msg);
//         }));
//     };
// }

pub fn init(
    source: &str,
    // url: Option<&str>,
    // api_key: &str,
    // log_level: Level,
) -> Result<()> {
    let log_level = super::helpers::get_log_level();
    // let logger = SeqLogger::new(source, url, api_key, log_level)?;
    let logger = SeqLogger::new(
        source,
        super::helpers::get_api_url().as_deref(),
        &super::helpers::get_api_key().ok_or(anyhow::anyhow!("API key not found"))?,
        log_level,
    )?;
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(log_level.to_level_filter());
    Ok(())
}

enum MessageOrExit {
    Message(serde_json::Value),
    Exit(Sender<()>),
}

struct SeqLogger {
    source: String,
    _thread: std::thread::JoinHandle<Result<()>>,
    send: Sender<MessageOrExit>,
    log_level: Level,
}

static mut SENDER: MaybeUninit<StaticSender> = MaybeUninit::uninit();

struct StaticSender {
    sender: Sender<MessageOrExit>,
    source: String,
}

impl StaticSender {
    fn write(&self, msg: serde_json::Value) {
        if let Err(e) = self.sender.send(MessageOrExit::Message(msg)) {
            eprintln!("Failed to send log message: {e}");
        }
    }
}

impl SeqLogger {
    fn new(source: &str, url: Option<&str>, api_key: &str, log_level: Level) -> Result<Self> {
        let mut url = match url {
            Some(u) => u.to_string(),
            None => "http://localhost:5341".to_string(),
        };
        let api_key = api_key.to_string();
        if !url.ends_with("/ingest/clef") {
            url.push_str("/ingest/clef");
        }
        let source = source.to_string();
        let (send, recv) = channel::<MessageOrExit>();
        // Store the sender in a static variable
        unsafe {
            SENDER.write(StaticSender {
                sender: send.clone(),
                source: source.clone(),
            });
        }
        let thread = {
            let source = source.clone();
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                if let Err(e) = post(
                    &client,
                    serde_json::json!({
                        "@t": chrono::Utc::now().to_rfc3339(),
                        "@l": "Info",
                        "@mt": "[{source}] {msg}",
                        "level": "Info",
                        "msg": "INITIALIZING LOGGER",
                        "source": source,
                        "thread": std::thread::current().name().unwrap_or("main"),
                    }),
                    &url,
                    &api_key,
                ) {
                    return Err(anyhow::anyhow!("Failed to initialize logger: {}", e));
                }

                loop {
                    match recv.recv() {
                        Ok(MessageOrExit::Message(info)) => {
                            if let Err(e) = post(&client, info, &url, &api_key) {
                                eprintln!("{e}");
                            }
                        }
                        Ok(MessageOrExit::Exit(sender)) => {
                            sender.send(()).ok();
                            eprintln!("Logger thread exiting");
                            break;
                        }
                        Err(e) => {
                            eprintln!("Failed to receive log message: {e}");
                            break;
                        }
                    }
                }
                Ok(())
            })
        };
        Ok(SeqLogger {
            source,
            _thread: thread,
            send,
            log_level,
        })
    }
}

impl log::Log for SeqLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.log_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let val = crate::helpers::parse_info(record, &self.source);
            match val {
                Ok(Some(val)) => {
                    if let Err(e) = self.send.send(MessageOrExit::Message(val)) {
                        eprintln!("Failed to send log: {e}");
                    }
                }
                Ok(None) => {
                    // Silently ignore.
                }
                Err(e) => {
                    eprintln!("Failed to parse log message: {e}");
                }
            }
        }
    }

    fn flush(&self) {
        let (sender, receiver) = channel();
        self.send.send(MessageOrExit::Exit(sender)).ok();
        if let Err(e) = receiver.recv() {
            eprintln!("Failed to receive exit signal: {e}");
        }
    }
}

fn post(
    client: &reqwest::blocking::Client,
    info: serde_json::Value,
    url: &str,
    api_key: &str,
) -> Result<()> {
    let client = client.clone();
    let url = url.to_string();
    let api_key = api_key.to_string();
    let res = match client
        .post(url)
        .header("X-Seq-ApiKey", api_key)
        .header("Content-Type", "application/json")
        .json(&info)
        .send()
    {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to send log: {e}");
            return Err(anyhow::anyhow!("Failed to send log: {e}"));
        }
    };
    if res.status().is_success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Failed to upload log: {}", res.status()))
    }
}

fn get_static_sender() -> &'static StaticSender {
    unsafe { SENDER.assume_init_ref() }
}

pub mod rich_anyhow_logging {
    pub fn error(err: &anyhow::Error) {
        with_level(log::Level::Error, err);
    }

    pub fn warn(err: &anyhow::Error) {
        with_level(log::Level::Warn, err);
    }

    pub fn info(err: &anyhow::Error) {
        with_level(log::Level::Info, err);
    }

    pub fn debug(err: &anyhow::Error) {
        with_level(log::Level::Debug, err);
    }

    pub fn trace(err: &anyhow::Error) {
        with_level(log::Level::Trace, err);
    }

    pub fn with_level(level: log::Level, err: &anyhow::Error) {
        let sender = super::get_static_sender();
        let trace = format!("{err:?}");
        let msg = format!("{err}");
        let val = serde_json::json!({
            "@t": chrono::Utc::now().to_rfc3339(),
            "@l": level.to_string(),
            "@mt": "[{source} | {level}] {msg}",
            "@x": trace,
            "level": level.to_string(),
            "msg": msg,
            "source": sender.source,
            "thread": std::thread::current().name().unwrap_or("main"),
        });
        sender.write(val);
    }
}
