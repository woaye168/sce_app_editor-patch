// host_stub.rs — 本地调试 host 桩：验证编辑器「调试(本地服务器)」全链
// 协议参照 mini-runtime src/core/host.rs（EditorLogin → 上传 → EditorStartGame）
// 用法：cargo run --example host_stub  → 然后编辑器 use_local_host 调试
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

fn put_varint(buf: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 { b |= 0x80; }
        buf.push(b);
        if v == 0 { break; }
    }
}
fn put_field_varint(buf: &mut Vec<u8>, field: u32, v: u64) {
    put_varint(buf, ((field << 3) | 0) as u64);
    put_varint(buf, v);
}
fn put_field_bytes(buf: &mut Vec<u8>, field: u32, data: &[u8]) {
    put_varint(buf, ((field << 3) | 2) as u64);
    put_varint(buf, data.len() as u64);
    buf.extend_from_slice(data);
}
fn encode_frame(msg_type: u64, body: &[u8]) -> Vec<u8> {
    let mut header = Vec::new();
    put_field_varint(&mut header, 1, msg_type);
    put_field_bytes(&mut header, 2, body);
    let mut env = Vec::new();
    put_field_bytes(&mut env, 1, &header);
    let total = 4 + 1 + env.len();
    let mut frame = Vec::with_capacity(total);
    frame.extend_from_slice(&(total as u32).to_le_bytes());
    frame.push(0);
    frame.extend_from_slice(&env);
    frame
}
fn get_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut v: u64 = 0;
    let mut shift = 0;
    loop {
        if *pos >= data.len() { return None; }
        let b = data[*pos];
        *pos += 1;
        v |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 { return Some(v); }
        shift += 7;
        if shift > 63 { return None; }
    }
}
fn decode_frame(data: &[u8]) -> Option<(u64, Vec<u8>)> {
    let mut pos = 4; // skip total
    pos += 1; // flag
    if get_varint(data, &mut pos)? != 0x0A { return None; }
    let elen = get_varint(data, &mut pos)? as usize;
    let env = &data[pos..pos + elen];
    let mut ep = 0;
    if get_varint(env, &mut ep)? != 0x08 { return None; }
    let msg = get_varint(env, &mut ep)?;
    if get_varint(env, &mut ep)? != 0x12 { return None; }
    let blen = get_varint(env, &mut ep)? as usize;
    Some((msg, env[ep..ep + blen].to_vec()))
}
fn body_bytes(body: &[u8], want: u32) -> Option<Vec<u8>> {
    let mut pos = 0;
    while pos < body.len() {
        let tag = get_varint(body, &mut pos)?;
        let field = (tag >> 3) as u32;
        match tag & 7 {
            0 => { get_varint(body, &mut pos)?; }
            2 => {
                let n = get_varint(body, &mut pos)? as usize;
                if field == want { return Some(body[pos..pos + n].to_vec()); }
                pos += n;
            }
            _ => return None,
        }
    }
    None
}

fn read_frame(s: &mut TcpStream) -> Option<Vec<u8>> {
    let mut lb = [0u8; 4];
    s.read_exact(&mut lb).ok()?;
    let total = u32::from_le_bytes(lb) as usize;
    let mut rest = vec![0u8; total - 4];
    s.read_exact(&mut rest).ok()?;
    let mut f = lb.to_vec();
    f.extend_from_slice(&rest);
    Some(f)
}
fn send(s: &mut TcpStream, msg: u64, body: &[u8]) {
    let f = encode_frame(msg, body);
    let _ = s.write_all(&f);
    let _ = s.flush();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:5003").expect("bind 5003");
    println!("LISTENING 127.0.0.1:5003");
    let (mut s, peer) = listener.accept().expect("accept");
    println!("ACCEPTED {peer}");
    s.set_read_timeout(Some(Duration::from_secs(600))).ok();
    s.set_nodelay(true).ok();

    let start = Instant::now();
    let mut files = 0u32;
    let mut blocks = 0u32;
    let mut started = false;
    loop {
        let frame = match read_frame(&mut s) {
            Some(f) => f,
            None => { println!("EOF/err after {:?}", start.elapsed()); break; }
        };
        let (msg, body) = match decode_frame(&frame) {
            Some(x) => x,
            None => { println!("BAD FRAME {} bytes", frame.len()); continue; }
        };
        match msg {
            0xF000 => {
                let token = body_bytes(&body, 2).map(|t| String::from_utf8_lossy(&t).to_string());
                println!("RECV EditorLogin token={token:?}");
                // EditorLoginResult { f1 varint result=0 }
                let mut b = Vec::new();
                put_field_varint(&mut b, 1, 0);
                send(&mut s, 0xF001, &b);
                println!("SENT EditorLoginResult result=0");
            }
            0xF004 => {
                files += 1;
                let path = body_bytes(&body, 1).map(|p| String::from_utf8_lossy(&p).to_string());
                let has_content = body_bytes(&body, 3).is_some();
                if files <= 5 || files % 200 == 0 {
                    println!("RECV SendWriteFile #{files} path={path:?} content={has_content}");
                }
                // SendWriteFileAck { f1 varint 0 } — 若无 ack 编辑器可能不发后续；先发为敬
                let mut b = Vec::new();
                put_field_varint(&mut b, 1, 0);
                send(&mut s, 0xF010, &b);
            }
            0xF008 => { blocks += 1; }
            0xF00A => {}
            0xF011 => {
                // EditorPingRes { f1 varint 0 }
                let mut b = Vec::new();
                put_field_varint(&mut b, 1, 0);
                send(&mut s, 0xF017, &b);
            }
            0xF012 => {
                let proj = body_bytes(&body, 1).map(|p| String::from_utf8_lossy(&p).to_string());
                println!("RECV EditorStartGame proj={proj:?} files={files} blocks={blocks}");
                // EditorStartGameRes { f1 varint result=0, f5 varint session_id }
                let mut b = Vec::new();
                put_field_varint(&mut b, 1, 0);
                put_field_varint(&mut b, 5, 123456789);
                send(&mut s, 0xF018, &b);
                println!("SENT EditorStartGameRes result=0 session=123456789");
                started = true;
            }
            _ => {
                println!("RECV msg=0x{msg:x} len={}", frame.len());
            }
        }
        if started {
            // 起局后保持连接继续收（ping/心跳），打一行状态
            println!("SESSION UP {:?} — 等待客户端连接/后续帧（Ctrl+C 结束）", start.elapsed());
            started = false;
        }
    }
}
