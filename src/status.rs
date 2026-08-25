use crate::config::Config;
use crate::logger::log;
use crate::protocol::{send_packet, write_string, SocketReader};
use std::io;
use std::net::TcpStream;

/// Handle the SLP (Server List Ping) flow.
/// - `stream`: mutable reference for writing responses.
/// - `reader`: reader for incoming packets (borrows a cloned read stream).
/// Returns Ok(()) on success or when a timeout occurs (handled internally).
/// Returns Err for other I/O errors.
pub fn handle_status(
    stream: &mut TcpStream,
    reader: &mut SocketReader,
    ip: &str,
    config: &Config,
) -> io::Result<()> {
    // 1. Receive Status Request
    let (pid, _payload) = match reader.read_packet() {
        Ok(packet) => packet,
        Err(e) if is_timeout_error(&e) => {
            log("WARN", ip, "Status request timed out");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    if pid != 0x00 {
        log("WARN", ip, &format!("Expected Status Request, got packet id {}, closing.", pid));
        return Ok(());
    }
    log("INFO", ip, "Received Status Request");

    // 2. Build players sample
    let mut sample = Vec::new();
    if !config.player_list.is_empty() {
        for name in config.player_list.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            sample.push(format!(
                r#"{{"name":"{}","id":"00000000-0000-0000-0000-000000000000"}}"#,
                name
            ));
        }
    }
    let sample_json = sample.join(",");

    // 3. Build JSON response (manually to avoid external crates)
    let json = format!(
        r#"{{"version":{{"name":"{}","protocol":{}}},"players":{{"max":{},"online":{},"sample":[{}]}},"description":{{"text":"{}"}}}}"#,
        config.game_version,
        config.protocol,
        config.max_players,
        config.online_players,
        sample_json,
        config.motd.replace('\\', "\\\\").replace('"', "\\\"")
    );

    // 4. Send Status Response
    let payload = write_string(&json);
    send_packet(stream, 0x00, &payload)?;
    log("INFO", ip, "Sent Status Response");

    // 5. Receive Ping Request
    let (pid, payload) = match reader.read_packet() {
        Ok(packet) => packet,
        Err(e) if is_timeout_error(&e) => {
            log("WARN", ip, "Ping request timed out");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    if pid != 0x01 {
        log("WARN", ip, &format!("Expected Ping Request, got packet id {}, closing.", pid));
        return Ok(());
    }
    log("INFO", ip, "Received Ping Request");
    if payload.len() != 8 {
        log("WARN", ip, &format!("Invalid ping payload length {}, expected 8.", payload.len()));
        return Ok(());
    }

    // 6. Send Pong (echo payload)
    send_packet(stream, 0x01, &payload)?;
    log("INFO", ip, "Sent Pong");
    Ok(())
}

/// Check if the error is a timeout (WouldBlock or TimedOut)
fn is_timeout_error(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut
}
