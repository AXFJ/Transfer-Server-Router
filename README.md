# Transfer Server Router by AXFJ

[中文](README_zh.md) | **English**

> Now rewritten in rust!

## Introduction

Transfer Server Router is an extremely lightweight Minecraft server routing tool. It sends a transfer packet as soon as a player connects to the server (equivalent to the vanilla `/transfer` command), quickly redirecting the player to the specified target server. This tool can be used for routing incoming connections at the server entry point, or in scenarios where **players are forced to use a public IP under a virtual LAN (please refer to my other repository `Radmin-VPN-Minecraft-Server-Force-Join-with-Public-IP`)**.

**Currently stable protocol support**: Minecraft 1.21.11 (protocol version 774), configurable for compatibility with other versions.

---

## Features

- Configurable connection limits:
  - Global maximum concurrent connections
  - Maximum concurrent connections per IP
  - Rate limit per IP (requests per second)
- Connection timeout setting to prevent zombie connections
- Protocol version mismatch warnings (configurable)
- Clean configuration file
- Structured logging with timestamps, levels, IPs, and messages

---

## Configuration File

On first startup, the program will automatically generate a `tsr_server.properties` file in the current directory with default settings. You can modify the parameters as needed.

### Configuration Options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `ip` | string | `0.0.0.0` | Listening IP address |
| `port` | integer | `25565` | Listening port |
| `target-ip` | string | `example.com` | Target server IP or domain name |
| `target-port` | integer | `25565` | Target server port |
| `protocol` | integer | `774` | Expected client protocol version. A warning is logged if the client version mismatches, but the connection is not rejected. |
| `max-conn` | integer | `5` | Global maximum concurrent connections |
| `max-conn-per-ip` | integer | `2` | Maximum concurrent connections per IP |
| `rate-per-ip` | float | `1.0` | Allowed requests per second per IP |
| `timeout-per-conn` | integer | `15` | Connection timeout in seconds |

---

## Requirements

- **Python 3.8+** (developed with Python 3.11.4)

---

## Quick Start

### 1. Prepare the script

Save `transfer.py` to any directory.

### 2. Start the service

In the terminal, run:

```bash
python transfer.py
```

On first startup, the default configuration file `tsr_server.properties` will be created automatically, and the service will start listening according to the settings.

### 3. Stop the service

Type `stop` in the running console and press Enter.

---

## License

Apache 2.0

---

## TODOs

Rewrite in Rust (maybe)
