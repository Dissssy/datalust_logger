#![feature(try_blocks)]
mod helpers;

// #[cfg(feature = "async")]
// mod l_async;
// #[cfg(feature = "async")]
// pub use l_async::init;

// #[cfg(not(feature = "async"))]
mod l_sync;
// #[cfg(not(feature = "async"))]
pub use l_sync::init;
pub use l_sync::rich_anyhow_logging;

#[cfg(test)]
mod tests {
    // spin up a server on a free port and register the logger with it
    use super::*;
    use std::sync::mpsc::{channel, Receiver};
    use std::thread;

    fn spin_server() -> (thread::JoinHandle<()>, Receiver<String>, u16) {
        let (tx, rx) = channel();
        let listener = std::net::TcpListener::bind("localhost:0").expect("Failed to bind to port");
        let port = listener.local_addr().unwrap().port();


        let handle = thread::spawn(move || {
            // start the server here
            // listen on the port and send messages to the channel
            for stream in listener.incoming() {
                let stream = stream.expect("Failed to accept connection");
                let mut buf = [0; 1024];
                let bytes_read = stream
                    .peek(&mut buf)
                    .expect("Failed to read from stream");
                let msg = String::from_utf8_lossy(&buf[..bytes_read]);
                // send the message to the channel
                let msg = msg.to_string();
                tx.send(msg).unwrap();
            }
        });
        (handle, rx, port)
    }

    fn init_logger(source: &str, port: u16) {
        // set the environment variables
        std::env::set_var("SEQ_API_KEY", "dummy_key");
        std::env::set_var("SEQ_API_URL", format!("http://localhost:{}", port));
        std::env::set_var("SEQ_LOG_LEVEL", "Debug");
        // initialize the logger
        assert!(init(source).is_ok());
    }

    #[test]
    fn test_panic() {
        let (_, rx, port) = spin_server();
        init_logger("test_panic", port);
        // set the panic handler
        set_panic_handler!("test_panic");
        // trigger a panic in a separate thread
        let panic_thread = thread::spawn(|| {
            panic!("This is a test panic");
        });
        // wait for the panic to occur
        let _ = panic_thread.join();
        // check if the message was sent to the server
        let msg = rx.recv_timeout(std::time::Duration::from_secs(5));
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert!(msg.contains("This is a test panic"));
    }

    #[test]
    fn test_logging() {
        let (_, rx, port) = spin_server();
        init_logger("test_logging", port);
        // log a message
        log::info!("This is a test log message");
        // check if the message was sent to the server
        let msg = rx.recv_timeout(std::time::Duration::from_secs(5));
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert!(msg.contains("This is a test log message"));
    }
}