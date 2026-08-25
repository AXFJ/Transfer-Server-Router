mod client_handler;
mod config;
mod logger;
mod protocol;
mod status;

use client_handler::handle_client;
use config::Config;
use logger::log;
use std::io::{self, BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

/// Shared global state for connection tracking and rate limiting.
struct SharedState {
    active_connections: Mutex<usize>,
    ip_connections: Mutex<std::collections::HashMap<String, usize>>,
    ip_last_request: Mutex<std::collections::HashMap<String, f64>>,
}

fn console_input_listener(listener: Arc<TcpListener>, running: Arc<AtomicBool>) {
    // Read lines from stdin in a loop
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    while running.load(Ordering::SeqCst) {
        // Print prompt
        print!("");
        if let Err(e) = io::stdout().flush() {
            // If flushing fails, we can still continue reading input
            eprintln!("Failed to flush stdout: {}", e);
        }

        match lines.next() {
            Some(Ok(line)) => {
                let trimmed = line.trim();
                if trimmed.eq_ignore_ascii_case("stop") {
                    log("INFO", "-", "Stopping server...");
                    // Signal all threads to stop
                    running.store(false, Ordering::SeqCst);

                    // Try to close the listener to break the blocking accept() in the main loop.
                    // If we can get exclusive ownership (no other Arc references), drop it to close.
                    // Otherwise, set non-blocking to force accept() to return an error.
                    match Arc::try_unwrap(listener.clone()) {
                        Ok(listener) => {
                            drop(listener); // Closes the socket
                        }
                        Err(arc) => {
                            // Still shared, set non-blocking mode; accept() will return WouldBlock
                            let _ = arc.set_nonblocking(true);
                        }
                    }
                    break;
                }
                // Ignore other commands (could be extended later)
            }
            Some(Err(e)) => {
                log("ERROR", "-", &format!("Error reading input: {}", e));
                break;
            }
            None => {
                // EOF reached, exit
                break;
            }
        }
    }
}

fn main() {
    println!("Transfer Server Router v1.2 by AXFJ");
    println!("See https://github.com/AXFJ/Transfer-Server-Router.");

    // Load configuration (global immutable after load)
    let config = Arc::new(Config::load("tsr_server.properties"));

    if config.protocol != 774 {
        log(
            "WARN",
            "-",
            &format!(
                "The protocol version in the configuration {} does not match the default version 774, which may cause compatibility issues.",
                config.protocol
            ),
        );
    }

    let listener = TcpListener::bind((config.ip.as_str(), config.port)).expect("Failed to bind");
    log(
        "INFO",
        "-",
        &format!(
            "Listen on: {}:{} -> {}:{}",
            config.ip, config.port, config.target_ip, config.target_port
        ),
    );
    log("INFO", "-", &format!("Protocol Version: {}", config.protocol));
    log(
        "INFO",
        "-",
        &format!(
            "SLP: MOTD=\"{}\", Players={}/{}, Version={}",
            config.motd, config.online_players, config.max_players, config.game_version
        ),
    );
    log(
        "INFO",
        "-",
        &format!(
            "Limitations: Total Concurrent={}, Per-IP Concurrent={}, Rate={} req/s, Timeout={}s",
            config.max_conn, config.max_conn_per_ip, config.rate_per_ip, config.timeout_per_conn
        ),
    );

    let listener = Arc::new(listener);
    let running = Arc::new(AtomicBool::new(true));

    // Spawn console input thread
    {
        let listener_clone = Arc::clone(&listener);
        let running_clone = Arc::clone(&running);
        thread::spawn(move || console_input_listener(listener_clone, running_clone));
    }

    // Shared state for connection tracking
    let state = Arc::new(SharedState {
        active_connections: Mutex::new(0),
        ip_connections: Mutex::new(std::collections::HashMap::new()),
        ip_last_request: Mutex::new(std::collections::HashMap::new()),
    });

    listener.set_nonblocking(true).expect("Failed to set non-blocking mode");

    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, addr)) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                let config_clone = Arc::clone(&config);
                let state_clone = Arc::clone(&state);
                thread::spawn(move || {
                    handle_client(stream, addr, config_clone, state_clone);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                log("ERROR", "-", &format!("Error occurred while accepting connection: {}", e));
                break;
            }
        }
    }

    log("INFO", "-", "Server closed.");
}
