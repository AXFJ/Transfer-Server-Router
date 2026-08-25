use std::io::{self, Read, Write};
use std::net::TcpStream;

/// Encode an integer as a Minecraft VarInt.
pub fn encode_varint(mut value: i32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut b = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            b |= 0x80;
        }
        out.push(b);
        if value == 0 {
            break;
        }
    }
    out
}

/// Decode a VarInt from a byte slice starting at `offset`, returning (value, new_offset).
pub fn decode_varint(data: &[u8], offset: usize) -> (i32, usize) {
    let mut value = 0;
    let mut shift = 0;
    let mut i = offset;
    loop {
        let b = data[i];
        i += 1;
        value |= ((b & 0x7F) as i32) << shift;
        if b & 0x80 == 0 {
            return (value, i);
        }
        shift += 7;
    }
}

/// Read a length-prefixed UTF-8 string from data at offset, returning (string, new_offset).
pub fn read_string(data: &[u8], offset: usize) -> (String, usize) {
    let (len, offset) = decode_varint(data, offset);
    let s = String::from_utf8_lossy(&data[offset..offset + len as usize]).into_owned();
    (s, offset + len as usize)
}

/// Encode a string as a length-prefixed UTF-8 byte vector.
pub fn write_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = encode_varint(bytes.len() as i32);
    out.extend_from_slice(bytes);
    out
}

/// Send a Minecraft packet with packet ID and payload.
pub fn send_packet(stream: &mut TcpStream, packet_id: i32, payload: &[u8]) -> io::Result<()> {
    let mut packet = encode_varint(packet_id);
    packet.extend_from_slice(payload);
    let mut full = encode_varint(packet.len() as i32);
    full.extend_from_slice(&packet);
    stream.write_all(&full)
}

/// Helper to read exact number of bytes from a TcpStream.
pub struct SocketReader<'a> {
    stream: &'a TcpStream,
}

impl<'a> SocketReader<'a> {
    pub fn new(stream: &'a TcpStream) -> Self {
        SocketReader { stream }
    }

    pub fn read_exact(&mut self, n: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; n];
        self.stream.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn read_varint(&mut self) -> io::Result<i32> {
        let mut value = 0;
        let mut shift = 0;
        loop {
            let byte = self.read_exact(1)?[0];
            value |= ((byte & 0x7F) as i32) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    /// Read a full packet, returning (packet_id, payload).
    pub fn read_packet(&mut self) -> io::Result<(i32, Vec<u8>)> {
        let length = self.read_varint()? as usize;
        let data = self.read_exact(length)?;
        let (packet_id, offset) = decode_varint(&data, 0);
        Ok((packet_id, data[offset..].to_vec()))
    }
}
