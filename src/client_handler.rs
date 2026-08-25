use crate::config::Config;
use crate::logger::log;
use crate::protocol::{decode_varint, read_string, send_packet, write_string, SocketReader};
use crate::status::handle_status;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::SharedState; // SharedState is defined in main.rs

/// Handle a single client connection.
pub fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    config: Arc<Config>,
    state: Arc<SharedState>,
) {
    let ip = addr.ip().to_string();
    let mut acquired_total = false;
    let mut acquired_ip = false;

    // 1. Check total connection limit
    {
        let mut active = state.active_connections.lock().unwrap();
        if *active >= config.max_conn {
            log("WARN", &ip, "Rejected: Reached total concurrent limit");
            drop(stream);
            return;
        }
        *active += 1;
        acquired_total = true;
    }

    // 2. Rate limit per IP
    let interval = 1.0 / config.rate_per_ip;
    {
        let mut last_requests = state.ip_last_request.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let last = last_requests.get(&ip).copied().unwrap_or(0.0);
        if now - last < interval {
            log("WARN", &ip, "Rejected: Exceeded rate limit");
            if acquired_total {
                let mut active = state.active_connections.lock().unwrap();
                *active -= 1;
            }
            drop(stream);
            return;
        }
        last_requests.insert(ip.clone(), now);
    }

    // 3. Per-IP connection limit
    {
        let mut ip_conns = state.ip_connections.lock().unwrap();
        let count = ip_conns.entry(ip.clone()).or_insert(0);
        if *count >= config.max_conn_per_ip {
            log("WARN", &ip, "Rejected: Reached per-IP concurrent limit");
            if acquired_total {
                let mut active = state.active_connections.lock().unwrap();
                *active -= 1;
            }
            drop(stream);
            return;
        }
        *count += 1;
        acquired_ip = true;
    }

    // Set read timeout on the original stream (affects reads)
    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(config.timeout_per_conn))) {
        log("ERROR", &ip, &format!("Failed to set timeout: {}", e));
    }

    // Create a clone of the stream for reading; original will be used for writing.
    let read_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            log("ERROR", &ip, &format!("Failed to clone stream for reading: {}", e));
            cleanup(&state, &ip, acquired_total, acquired_ip);
            return;
        }
    };

    // Make the original stream mutable for writing
    let mut stream = stream;
    // Create a SocketReader that borrows the read clone
    let mut reader = SocketReader::new(&read_stream);

    // 1) Handshake
    match reader.read_packet() {
        Ok((pid, payload)) => {
            log("INFO", &ip, &format!("Received Handshake, packet_id={}", pid));
            if pid != 0x00 {
                cleanup(&state, &ip, acquired_total, acquired_ip);
                return;
            }

            let offset = 0;
            let (protocol_ver, offset) = decode_varint(&payload, offset);
            let (_server_addr, offset) = read_string(&payload, offset);
            let offset = offset + 2; // skip port (unsigned short)
            let (next_state, _) = decode_varint(&payload, offset);

            log("INFO", &ip, &format!("Protocol version={}, next state={}", protocol_ver, next_state));

            if next_state == 1 {
                // SLP (Server List Ping)
                if let Err(e) = handle_status(&mut stream, &mut reader, &ip, &config) {
                    log("ERROR", &ip, &format!("Error handling status: {}", e));
                }
                cleanup(&state, &ip, acquired_total, acquired_ip);
                return;
            } else if next_state == 2 {
                // Login flow

                // 2) Login Start
                match reader.read_packet() {
                    Ok((pid, payload)) => {
                        log("INFO", &ip, &format!("Received Login Start, packet_id={}", pid));
                        if pid != 0x00 {
                            cleanup(&state, &ip, acquired_total, acquired_ip);
                            return;
                        }

                        let offset = 0;
                        let (username, offset) = read_string(&payload, offset);
                        if payload.len() - offset < 16 {
                            log("ERROR", &ip, "An internal error occurred when decoding packets.");
                            cleanup(&state, &ip, acquired_total, acquired_ip);
                            return;
                        }
                        let uuid_bytes: [u8; 16] = payload[offset..offset + 16].try_into().unwrap();
                        let uuid = u128::from_be_bytes(uuid_bytes);

                        log("INFO", &ip, &format!("Player \"{}\" UUID={:032x}", username, uuid));

                        // 3) Login Success
                        let mut login_payload = Vec::new();
                        login_payload.extend_from_slice(&uuid_bytes);
                        login_payload.extend_from_slice(&write_string(&username));
                        login_payload.extend_from_slice(&crate::protocol::encode_varint(0));
                        if let Err(e) = send_packet(&mut stream, 0x02, &login_payload) {
                            log("ERROR", &ip, &format!("Failed to send Login Success: {}", e));
                            cleanup(&state, &ip, acquired_total, acquired_ip);
                            return;
                        }
                        log("INFO", &ip, "Sent Login Success");

                        // 4) Send Transfer
                        let mut transfer_payload = write_string(&config.target_ip);
                        transfer_payload.extend_from_slice(&crate::protocol::encode_varint(config.target_port as i32));
                        if let Err(e) = send_packet(&mut stream, 0x0B, &transfer_payload) {
                            log("ERROR", &ip, &format!("Failed to send Transfer: {}", e));
                        } else {
                            log("INFO", &ip, &format!("Sent Transfer -> {}:{}", config.target_ip, config.target_port));
                        }

                        // Wait 2 seconds as in original
                        std::thread::sleep(Duration::from_secs(2));
                    }
                    Err(e) if is_timeout_error(&e) => {
                        log("WARN", &ip, "Login Start timed out");
                    }
                    Err(e) => {
                        log("ERROR", &ip, &format!("Error reading Login Start: {}", e));
                    }
                }
            } else {
                log("WARN", &ip, &format!("Unsupported next state {}, closing.", next_state));
            }
        }
        Err(e) if is_timeout_error(&e) => {
            log("WARN", &ip, "Handshake timed out");
        }
        Err(e) => {
            log("ERROR", &ip, &format!("Error reading handshake: {}", e));
        }
    }

    cleanup(&state, &ip, acquired_total, acquired_ip);
}

/// Check if the error is a timeout (WouldBlock or TimedOut)
fn is_timeout_error(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut
}

/// Decrement connection counters and release per-IP slot
fn cleanup(state: &Arc<SharedState>, ip: &str, acquired_total: bool, acquired_ip: bool) {
    if acquired_ip {
        let mut ip_conns = state.ip_connections.lock().unwrap();
        if let Some(count) = ip_conns.get_mut(ip) {
            *count -= 1;
            if *count <= 0 {
                ip_conns.remove(ip);
            }
        }
    }
    if acquired_total {
        let mut active = state.active_connections.lock().unwrap();
        *active -= 1;
    }
}
