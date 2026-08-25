use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn log(level: &str, ip: &str, msg: &str) {
    // Format current time as HH:MM:SS
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let hours = (now / 3600) % 24;
    let minutes = (now / 60) % 60;
    let seconds = now % 60;
    let timestamp = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

    let ip_display = if ip.is_empty() { "-" } else { ip };
    let mut log_msg = format!("[{}] [{}] [{}] {}", timestamp, level, ip_display, msg);

    match level {
        "WARN" => log_msg = format!("\x1b[93m{}\x1b[0m", log_msg),
        "ERROR" => log_msg = format!("\x1b[91m{}\x1b[0m", log_msg),
        _ => {}
    }

    let _guard = LOG_LOCK.lock().unwrap();
    println!("{}", log_msg);
}
