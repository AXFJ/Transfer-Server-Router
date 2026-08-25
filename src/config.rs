use crate::logger::log;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Configuration structure, mirrors DEFAULT_CONFIG in the Python script.
pub struct Config {
    pub ip: String,
    pub port: u16,
    pub target_ip: String,
    pub target_port: u16,
    pub protocol: i32,
    pub max_conn: usize,
    pub max_conn_per_ip: usize,
    pub rate_per_ip: f64,
    pub timeout_per_conn: u64,
    pub motd: String,
    pub online_players: i32,
    pub max_players: i32,
    pub game_version: String,
    pub player_list: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ip: "0.0.0.0".to_string(),
            port: 25565,
            target_ip: "example.com".to_string(),
            target_port: 25565,
            protocol: 774,
            max_conn: 5,
            max_conn_per_ip: 2,
            rate_per_ip: 1.0,
            timeout_per_conn: 15,
            motd: "A Minecraft Server".to_string(),
            online_players: 0,
            max_players: 20,
            game_version: "1.21.11".to_string(),
            player_list: String::new(),
        }
    }
}

impl Config {
    /// Load configuration from file, creating a default file if not present.
    pub fn load(filepath: &str) -> Self {
        let mut config = Config::default();
        if !Path::new(filepath).exists() {
            log(
                "INFO",
                "-",
                &format!(
                    "Configuration file {} does not exist, using default configuration and creating default file.",
                    filepath
                ),
            );
            let mut default_content = String::new();
            for (key, value) in default_as_key_values() {
                default_content.push_str(&format!("{}={}\n", key, value));
            }
            if let Err(e) = File::create(filepath).and_then(|mut f| f.write_all(default_content.as_bytes())) {
                log(
                    "ERROR",
                    "-",
                    &format!("Failed to create default configuration file: {}", e),
                );
            }
            return config;
        }

        match File::open(filepath) {
            Ok(file) => {
                let reader = BufReader::new(file);
                for (line_num, line) in reader.lines().enumerate() {
                    let line_num = line_num + 1;
                    match line {
                        Ok(line) => {
                            let line = line.trim();
                            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                                continue;
                            }
                            if !line.contains('=') {
                                log(
                                    "WARN",
                                    "-",
                                    &format!(
                                        "Configuration file line {} has invalid format, skipping.",
                                        line_num
                                    ),
                                );
                                continue;
                            }
                            let mut parts = line.splitn(2, '=');
                            let key = parts.next().unwrap().trim().to_lowercase();
                            let value = parts.next().unwrap().trim();
                            match key.as_str() {
                                "ip" => config.ip = value.to_string(),
                                "port" => config.port = parse_value(value, config.port, "port"),
                                "target-ip" => config.target_ip = value.to_string(),
                                "target-port" => config.target_port = parse_value(value, config.target_port, "target-port"),
                                "protocol" => config.protocol = parse_value(value, config.protocol, "protocol"),
                                "max-conn" => config.max_conn = parse_value(value, config.max_conn, "max-conn"),
                                "max-conn-per-ip" => config.max_conn_per_ip = parse_value(value, config.max_conn_per_ip, "max-conn-per-ip"),
                                "rate-per-ip" => config.rate_per_ip = parse_value(value, config.rate_per_ip, "rate-per-ip"),
                                "timeout-per-conn" => config.timeout_per_conn = parse_value(value, config.timeout_per_conn, "timeout-per-conn"),
                                "motd" => config.motd = value.to_string(),
                                "online-players" => config.online_players = parse_value(value, config.online_players, "online-players"),
                                "max-players" => config.max_players = parse_value(value, config.max_players, "max-players"),
                                "game-version" => config.game_version = value.to_string(),
                                "player-list" => config.player_list = value.to_string(),
                                _ => {
                                    log("WARN", "-", &format!("Unknown configuration key '{}', ignored.", key));
                                }
                            }
                        }
                        Err(e) => {
                            log("ERROR", "-", &format!("Failed to read configuration file: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                log(
                    "ERROR",
                    "-",
                    &format!("Failed to read configuration file: {}, using default configuration.", e),
                );
                config = Config::default();
            }
        }
        config
    }
}

fn default_as_key_values() -> Vec<(&'static str, String)> {
    let d = Config::default();
    vec![
        ("ip", d.ip),
        ("port", d.port.to_string()),
        ("target-ip", d.target_ip),
        ("target-port", d.target_port.to_string()),
        ("protocol", d.protocol.to_string()),
        ("max-conn", d.max_conn.to_string()),
        ("max-conn-per-ip", d.max_conn_per_ip.to_string()),
        ("rate-per-ip", d.rate_per_ip.to_string()),
        ("timeout-per-conn", d.timeout_per_conn.to_string()),
        ("motd", d.motd),
        ("online-players", d.online_players.to_string()),
        ("max-players", d.max_players.to_string()),
        ("game-version", d.game_version),
        ("player-list", d.player_list),
    ]
}

fn parse_value<T: std::str::FromStr>(value: &str, default: T, key: &str) -> T {
    match value.parse::<T>() {
        Ok(v) => v,
        Err(_) => {
            log(
                "WARN",
                "-",
                &format!("Invalid value '{}' for key '{}', using default value.", value, key),
            );
            default
        }
    }
}
