use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Serialize, Clone)]
pub struct PingResult {
    pub latency_ms: u64,
    pub version: String,
    pub protocol: i64,
    pub online: i64,
    pub max: i64,
    pub motd: String,
    pub favicon: Option<String>,
}

#[tauri::command]
pub async fn ping_server(host: String, port: Option<u16>) -> Result<PingResult, String> {
    let port = port.unwrap_or(25565);
    let addr = format!("{host}:{port}");

    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        do_ping(stream, &host, port).await
    })
    .await
    .map_err(|_| "timeout".to_string())?
}

async fn do_ping(mut s: TcpStream, host: &str, port: u16) -> Result<PingResult, String> {
    {
        let mut body = Vec::new();
        write_varint(&mut body, 0x00);
        write_varint(&mut body, -1);
        write_string(&mut body, host);
        body.extend_from_slice(&port.to_be_bytes());
        write_varint(&mut body, 1);
        send_packet(&mut s, &body).await?;
    }

    {
        let mut body = Vec::new();
        write_varint(&mut body, 0x00);
        send_packet(&mut s, &body).await?;
    }

    let json_str = {
        let body = recv_packet(&mut s).await?;
        let mut cur = 0usize;
        let _pkt_id = read_varint_buf(&body, &mut cur)?;
        let str_len = read_varint_buf(&body, &mut cur)? as usize;
        let end = cur + str_len;
        let bytes = body.get(cur..end).ok_or("status response truncated")?;
        String::from_utf8_lossy(bytes).into_owned()
    };

    let latency_ms = {
        let mut body = Vec::new();
        write_varint(&mut body, 0x01);
        body.extend_from_slice(&1i64.to_be_bytes());
        let t = std::time::Instant::now();
        send_packet(&mut s, &body).await?;
        let _ = recv_packet(&mut s).await;
        t.elapsed().as_millis() as u64
    };

    parse_status(&json_str, latency_ms)
}

fn parse_status(json: &str, latency_ms: u64) -> Result<PingResult, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("bad JSON: {e}"))?;

    let version = v["version"]["name"].as_str().unwrap_or("?").to_string();
    let protocol = v["version"]["protocol"].as_i64().unwrap_or(-1);
    let online = v["players"]["online"].as_i64().unwrap_or(0);
    let max = v["players"]["max"].as_i64().unwrap_or(0);
    let motd = flatten_chat(&v["description"]);
    let favicon = v["favicon"].as_str().map(str::to_string);

    Ok(PingResult { latency_ms, version, protocol, online, max, motd, favicon })
}

fn flatten_chat(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => strip_codes(s),
        serde_json::Value::Object(obj) => {
            let mut out = obj
                .get("text")
                .and_then(|t| t.as_str())
                .map(strip_codes)
                .unwrap_or_default();
            if let Some(extra) = obj.get("extra") {
                out.push_str(&flatten_chat(extra));
            }
            out
        }
        serde_json::Value::Array(arr) => arr.iter().map(flatten_chat).collect(),
        _ => String::new(),
    }
}

fn strip_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
            continue;
        }
        if c == '§' || c == '\u{00a7}' {
            skip = true;
            continue;
        }
        out.push(c);
    }
    out
}

async fn send_packet(s: &mut TcpStream, body: &[u8]) -> Result<(), String> {
    let mut packet = Vec::with_capacity(5 + body.len());
    write_varint(&mut packet, body.len() as i32);
    packet.extend_from_slice(body);
    s.write_all(&packet).await.map_err(|e| e.to_string())
}

async fn recv_packet(s: &mut TcpStream) -> Result<Vec<u8>, String> {
    let len = read_varint_async(s).await? as usize;
    let mut buf = vec![0u8; len];
    s.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
    Ok(buf)
}

async fn read_varint_async(s: &mut TcpStream) -> Result<i32, String> {
    let mut result = 0i32;
    let mut shift = 0u32;
    loop {
        let mut byte = [0u8; 1];
        s.read_exact(&mut byte).await.map_err(|e| e.to_string())?;
        let b = byte[0];
        result |= ((b & 0x7F) as i32) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            return Ok(result);
        }
        if shift >= 35 {
            return Err("VarInt too large".into());
        }
    }
}

fn read_varint_buf(buf: &[u8], cur: &mut usize) -> Result<i32, String> {
    let mut result = 0i32;
    let mut shift = 0u32;
    loop {
        let b = *buf.get(*cur).ok_or("unexpected end of buffer")?;
        *cur += 1;
        result |= ((b & 0x7F) as i32) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            return Ok(result);
        }
        if shift >= 35 {
            return Err("VarInt too large".into());
        }
    }
}

fn write_varint(buf: &mut Vec<u8>, mut val: i32) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val = ((val as u32) >> 7) as i32;
        if val != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if val == 0 {
            break;
        }
    }
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_varint(buf, s.len() as i32);
    buf.extend_from_slice(s.as_bytes());
}
