##########################
#                        #
# Transfer Server Router #
#         v1.0           #
#        by AXFJ         #
#                        #
##########################+

import socket
import threading
import uuid
import time
import os
from datetime import datetime

# Defaut configs
DEFAULT_CONFIG = {
    'ip': '0.0.0.0',
    'port': 25565,
    'target-ip': 'example.com',
    'target-port': 25565,
    'protocol': 774, # Recommended protocol version for 1.21.11
    'max-conn': 5,
    'max-conn-per-ip': 2,
    'rate-per-ip': 1.0,
    'timeout-per-conn': 15,
}

CONFIG = DEFAULT_CONFIG.copy()

# Global status variables
active_connections = 0
total_lock = threading.Lock()

ip_connections = {}
ip_lock = threading.Lock()

ip_last_request = {}
rate_lock = threading.Lock()

log_lock = threading.Lock()
running = True

# Logger
def log(level: str, ip: str, msg: str):
    """Format: [Timestamp] [Level] [IP] Message"""
    timestamp = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
    ip_display = ip if ip else '-'
    with log_lock:
        print(f'[{timestamp}] [{level}] [{ip_display}] {msg}')

# Load configuration from file
def load_config(filepath):
    global CONFIG
    if not os.path.exists(filepath):
        log('INFO', '-', f'Configuration file {filepath} does not exist, using default configuration and creating default file.')
        try:
            with open(filepath, 'w', encoding='utf-8') as f:
                for k, v in DEFAULT_CONFIG.items():
                    f.write(f'{k}={v}\n')
        except Exception as e:
            log('ERROR', '-', f'Failed to create default configuration file: {e}')
        CONFIG = DEFAULT_CONFIG.copy()
        return

    CONFIG = DEFAULT_CONFIG.copy()
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            for line_num, line in enumerate(f, 1):
                line = line.strip()
                if not line or line.startswith('#') or line.startswith('!'):
                    continue
                if '=' not in line:
                    log('WARN', '-', f'Configuration file line {line_num} has invalid format, skipping.')
                    continue
                key, value = line.split('=', 1)
                key = key.strip().lower()
                value = value.strip()
                if key in CONFIG:
                    default_val = CONFIG[key]
                    try:
                        if isinstance(default_val, int):
                            CONFIG[key] = int(value)
                        elif isinstance(default_val, float):
                            CONFIG[key] = float(value)
                        else:
                            CONFIG[key] = value
                    except ValueError:
                        log('WARN', '-', f"Invalid value '{value}' for key '{key}', using default value {default_val}.")
                else:
                    log('WARN', '-', f"Unknown configuration key '{key}', ignored.")
    except Exception as e:
        log('ERROR', '-', f'Failed to read configuration file: {e}, using default configuration.')
        CONFIG = DEFAULT_CONFIG.copy()

# Protocol encoding/decoding functions
def encode_varint(value: int) -> bytes:
    out = bytearray()
    while True:
        b = value & 0x7F
        value >>= 7
        if value:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)

def decode_varint(data: bytes, offset: int):
    value = 0
    shift = 0
    while True:
        b = data[offset]
        offset += 1
        value |= (b & 0x7F) << shift
        if not (b & 0x80):
            return value, offset
        shift += 7

def read_string(data: bytes, offset: int):
    n, offset = decode_varint(data, offset)
    s = data[offset:offset + n].decode('utf-8', errors='replace')
    return s, offset + n

def write_string(s: str) -> bytes:
    b = s.encode('utf-8')
    return encode_varint(len(b)) + b

def send_packet(sock: socket.socket, packet_id: int, payload: bytes = b''):
    packet = encode_varint(packet_id) + payload
    sock.sendall(encode_varint(len(packet)) + packet)

class SocketReader:
    def __init__(self, sock: socket.socket):
        self.sock = sock

    def read_exact(self, n: int) -> bytes:
        data = b''
        while len(data) < n:
            chunk = self.sock.recv(n - len(data))
            if not chunk:
                raise EOFError('connection closed')
            data += chunk
        return data

    def read_varint(self) -> int:
        value = 0
        shift = 0
        while True:
            b = self.read_exact(1)[0]
            value |= (b & 0x7F) << shift
            if not (b & 0x80):
                return value
            shift += 7

    def read_packet(self):
        length = self.read_varint()
        data = self.read_exact(length)
        packet_id, offset = decode_varint(data, 0)
        return packet_id, data[offset:]

# Client handler
def handle_client(sock: socket.socket, addr):
    global active_connections
    ip = addr[0]
    acquired_total = False
    acquired_ip = False

    # Max connections check
    with total_lock:
        if active_connections >= CONFIG['max-conn']:
            log('WARN', ip, 'Rejected: Reached total concurrent limit')
            sock.close()
            return
        active_connections += 1
        acquired_total = True

    # Max rate per ip check
    interval = 1.0 / CONFIG['rate-per-ip']
    with rate_lock:
        now = time.time()
        last = ip_last_request.get(ip, 0)
        if now - last < interval:
            log('WARN', ip, 'Rejected: Exceeded rate limit')
            with total_lock:
                active_connections -= 1
            sock.close()
            return
        ip_last_request[ip] = now

    # Max connections per ip check
    with ip_lock:
        if ip_connections.get(ip, 0) >= CONFIG['max-conn-per-ip']:
            log('WARN', ip, 'Rejected: Reached per-IP concurrent limit')
            with total_lock:
                active_connections -= 1
            sock.close()
            return
        ip_connections[ip] = ip_connections.get(ip, 0) + 1
        acquired_ip = True

    # Handle the handshake and transfer
    try:
        sock.settimeout(CONFIG['timeout-per-conn'])
        reader = SocketReader(sock)

        # 1) Handshake
        pid, payload = reader.read_packet()
        log('INFO', ip, f'Received Handshake, packet_id={pid}')
        if pid != 0x00:
            return

        offset = 0
        protocol_ver, offset = decode_varint(payload, offset)
        _, offset = read_string(payload, offset)
        offset += 2
        next_state, _ = decode_varint(payload, offset)

        log('INFO', ip, f'Protocol version={protocol_ver}, next state={next_state}')
        # Warn if protocol version does not match, but continue processing
        if protocol_ver != CONFIG['protocol']:
            log('WARN', ip, f'Client protocol version {protocol_ver} does not match configuration version {CONFIG["protocol"]}, continuing processing (may be compatible)')
        if next_state != 2:
            return

        # 2) Login Start
        pid, payload = reader.read_packet()
        log('INFO', ip, f'Received Login Start, packet_id={pid}')
        if pid != 0x00:
            return

        offset = 0
        username, offset = read_string(payload, offset)
        if len(payload) - offset < 16:
            log('ERROR', ip, 'An internal error occurred when decoding packets.')
            return
        player_uuid = uuid.UUID(bytes=payload[offset:offset + 16])

        log('INFO', ip, f'Player "{username}" UUID={player_uuid}')

        # 3) Login Success
        login_payload = player_uuid.bytes + write_string(username) + encode_varint(0)
        send_packet(sock, 0x02, login_payload)
        log('INFO', ip, 'Sent Login Success')

        # 4) Send Transfer
        transfer_payload = write_string(CONFIG['target-ip']) + encode_varint(CONFIG['target-port'])
        send_packet(sock, 0x0B, transfer_payload)
        log('INFO', ip, f'Sent Transfer -> {CONFIG["target-ip"]}:{CONFIG["target-port"]}')

        time.sleep(2)

    except Exception as e:
        log('ERROR', ip, f'Error: {e}')
    finally:
        if acquired_ip:
            with ip_lock:
                ip_connections[ip] = ip_connections.get(ip, 0) - 1
                if ip_connections[ip] <= 0:
                    del ip_connections[ip]
        if acquired_total:
            with total_lock:
                active_connections -= 1
        sock.close()

# Listen commands
def console_input_listener(server_sock):
    global running
    while running:
        try:
            cmd = input()
            if cmd.strip().lower() == 'stop':
                log('INFO', '-', 'Stopping server...')
                running = False
                server_sock.close()
                break
        except EOFError:
            break
        except Exception as e:
            log('ERROR', '-', f'Error: {e}')

# Main
def main():
    print('Transfer Server Router v1.0 by AXFJ')
    print('See https://github.com/AXFJ/Transfer-Server-Router.')

    global running
    load_config('tsr_server.properties')

    if CONFIG['protocol'] != 774:
        log('WARN', '-', f'The protocol version in the configuration {CONFIG["protocol"]} does not match the default version 774, which may cause compatibility issues.')

    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind((CONFIG['ip'], CONFIG['port']))
    server.listen(128)

    log('INFO', '-', f'Listen on：{CONFIG["ip"]}:{CONFIG["port"]} -> {CONFIG["target-ip"]}:{CONFIG["target-port"]}')
    log('INFO', '-', f'Protocol Version：{CONFIG["protocol"]}')
    log('INFO', '-', f'Limitations：Total Concurrent={CONFIG["max-conn"]}, Per-IP Concurrent={CONFIG["max-conn-per-ip"]}, '
                     f'Rate={CONFIG["rate-per-ip"]} req/s, Timeout={CONFIG["timeout-per-conn"]}s')

    input_thread = threading.Thread(target=console_input_listener, args=(server,), daemon=True)
    input_thread.start()

    while running:
        try:
            sock, addr = server.accept()
            if not running:
                break
            threading.Thread(target=handle_client, args=(sock, addr), daemon=True).start()
        except OSError:
            if not running:
                break
            else:
                log('ERROR', '-', 'Error occurred while accepting connection')
                break
        except Exception as e:
            log('ERROR', '-', f'Error occurred while accepting connection: {e}')
            break

    log('INFO', '-', 'Server closed.')

if __name__ == '__main__':
    main()
    