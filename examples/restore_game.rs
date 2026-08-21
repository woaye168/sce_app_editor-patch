//! SCE 加密包一键还原工具：TNND 解密 → 7z 解压 → UPAK 解包 → 图片(KTX)还原。
//! 用法：cargo run --example restore_game -- <加密7z路径> [-o 输出目录] [--keep-temp] [--no-decode-images]
//!
//! 流程说明：
//!   1. 输入文件若以 TNND 开头，先按 CREATEEASY 循环 XOR 解密为 7z；
//!      无 TNND 头则直接当 7z 处理。
//!   2. 解压 7z（系统 7z.exe / 7za / Windows 自带 tar）。
//!   3. 对其中的 .pak（UPAK 格式）逐条目解出完整文件树；
//!      其它文件若是 TNND 加密则顺带解密，否则原样拷贝。
//!   4. 扫描产物中的伪 KTX 图片（魔数 \xABKTX 11\xBB，BC7/BC1/BC2/BC3/RGBA8/RGB8），
//!      用 bcdec_rs 解码并就地还原为真正的 PNG（比旧 Python 版多支持 BC2/DXT3）。
//!
//! 只读研究工具：不修改输入文件。

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const TNND_MAGIC: &[u8; 4] = b"TNND";
const TNND_KEY: &[u8; 10] = b"CREATEEASY";
const UPAK_MAGIC: &[u8; 4] = b"UPAK";
const CHUNK: usize = 4 * 1024 * 1024;

// 伪 KTX 纹理（.png/.tga 扩展名的加密图片）
const KTX_MAGIC: &[u8; 12] = b"\xabKTX 11\xbb\r\n\x1a\n";
const IFMT_BC7: u32 = 0x8E8C; // GL_COMPRESSED_RGBA_BPTC_UNORM
const IFMT_RGBA8: u32 = 0x8058; // GL_RGBA8
const IFMT_RGB8: u32 = 0x8051; // GL_RGB8
const IFMT_DXT1: u32 = 0x83F1; // GL_COMPRESSED_RGBA_S3TC_DXT1_EXT (BC1)
const IFMT_DXT3: u32 = 0x83F2; // GL_COMPRESSED_RGBA_S3TC_DXT3_EXT (BC2)
const IFMT_DXT5: u32 = 0x83F3; // GL_COMPRESSED_RGBA_S3TC_DXT5_EXT (BC3)

// ---------------- TNND ----------------

fn is_tnnd(path: &Path) -> bool {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut head = [0u8; 4];
    f.read_exact(&mut head).is_ok() && &head == TNND_MAGIC
}

/// TNND 解密（流式 XOR）。返回写出字节数。调用方需先确认 is_tnnd。
fn tnnd_decrypt_file(src: &Path, dst: &Path) -> std::io::Result<u64> {
    let mut fin = fs::File::open(src)?;
    let mut fout = fs::File::create(dst)?;
    let mut head = [0u8; 4];
    fin.read_exact(&mut head)?;
    assert!(&head == TNND_MAGIC, "不是 TNND 文件: {}", src.display());
    let mut written = 0u64;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = fin.read(&mut buf)?;
        if n == 0 {
            break;
        }
        for (i, b) in buf[..n].iter_mut().enumerate() {
            *b ^= TNND_KEY[(written as usize + i) % TNND_KEY.len()];
        }
        fout.write_all(&buf[..n])?;
        written += n as u64;
    }
    Ok(written)
}

// ---------------- 7z ----------------

/// 解压 7z：7z / 7za / 7-Zip 安装目录 → Windows 自带 tar（bsdtar 支持 7z）。
fn extract_7z(archive: &Path, outdir: &Path) {
    let candidates = [
        "7z",
        "7za",
        r"C:\Program Files\7-Zip\7z.exe",
        r"C:\Program Files (x86)\7-Zip\7z.exe",
    ];
    for exe in candidates {
        if let Ok(status) = Command::new(exe)
            .arg("x")
            .arg(archive)
            .arg(format!("-o{}", outdir.display()))
            .arg("-y")
            .status()
        {
            if status.success() {
                return;
            }
        }
    }
    if let Ok(status) = Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(outdir)
        .status()
    {
        if status.success() {
            return;
        }
    }
    panic!("找不到可用的 7z 解压方式：请安装 7-Zip");
}

// ---------------- UPAK ----------------

fn u32le(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

/// 防路径穿越：去掉盘符/绝对路径，.. 替换为 __。
fn safe_rel(name: &str) -> PathBuf {
    let mut parts: Vec<String> = name
        .replace('\\', "/")
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .map(|p| if p == ".." { "__".into() } else { p.into() })
        .collect();
    if let Some(first) = parts.first_mut() {
        *first = first.replace(':', "_");
    }
    if parts.is_empty() {
        PathBuf::from("_unnamed")
    } else {
        parts.iter().collect()
    }
}

/// 解包 SCE UPAK（条目 = 名字\0 + u32 offset + u32 size + u32 checksum）。返回条目数。
fn upak_extract(pak_path: &Path, outdir: &Path) -> usize {
    let data = fs::read(pak_path).unwrap();
    assert!(&data[0..4] == UPAK_MAGIC, "不是 UPAK 文件: {}", pak_path.display());
    let count = u32le(&data, 4) as usize;
    // 偏移 8 为 u32 总校验，索引区从 12 开始
    let mut p = 12usize;
    let mut ok = 0usize;
    for _ in 0..count {
        let end = data[p..]
            .iter()
            .position(|&b| b == 0)
            .expect("条目名缺少 \\0 结尾，索引损坏")
            + p;
        let name = String::from_utf8_lossy(&data[p..end]).into_owned();
        p = end + 1;
        let offset = u32le(&data, p) as usize;
        let size = u32le(&data, p + 4) as usize;
        p += 12; // offset + size + checksum（比标准 Urho3D 多 4 字节校验）
        let target = outdir.join(safe_rel(&name));
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, &data[offset..offset + size]).unwrap();
        ok += 1;
    }
    ok
}

// ---------------- 图片还原（伪 KTX → PNG） ----------------

/// BC 压缩数据整图解码为 RGBA8。bcdec_rs 单块解码，按块拼装并裁剪到 w*h。
fn decode_bc(
    ifmt: u32,
    data: &[u8],
    w: usize,
    h: usize,
    src: &Path,
) -> Result<Vec<u8>, String> {
    let (block_size, f): (usize, fn(&[u8], &mut [u8], usize)) = match ifmt {
        IFMT_BC7 => (16, bcdec_rs::bc7),
        IFMT_DXT1 => (8, bcdec_rs::bc1),
        IFMT_DXT3 => (16, bcdec_rs::bc2),
        IFMT_DXT5 => (16, bcdec_rs::bc3),
        _ => unreachable!(),
    };
    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    let expected = bw * bh * block_size;
    if data.len() != expected {
        return Err(format!(
            "数据长度异常: {} != {expected} (0x{ifmt:04x}, {w}x{h}, {})",
            data.len(),
            src.display()
        ));
    }
    // bcdec 整块写 4x4，边缘块会越界，故按对齐尺寸解码再裁剪
    let pw = bw * 4;
    let pitch = pw * 4;
    let mut padded = vec![0u8; pitch * bh * 4];
    for by in 0..bh {
        for bx in 0..bw {
            let block = &data[(by * bw + bx) * block_size..][..block_size];
            f(block, &mut padded[by * 4 * pitch + bx * 16..], pitch);
        }
    }
    // 裁剪到 w*h 紧凑 RGBA（bcdec 输出即 RGBA，无需 R/B 互换）
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&padded[y * pitch..y * pitch + w * 4]);
    }
    Ok(out)
}

/// 解码单个伪 KTX 文件并就地保存为 PNG。非 KTX 文件返回 Ok(false)。
///
/// 格式（伪装成 KTX 纹理）：
///   - 12 字节魔数: AB 4B 54 58 20 31 31 BB 0D 0A 1A 0A
///   - 偏移 28: glInternalFormat（0x8E8C=BC7, 0x8058=RGBA8, 0x8051=RGB8,
///               0x83F1=DXT1/BC1, 0x83F2=DXT3/BC2, 0x83F3=DXT5/BC3）
///   - 偏移 36/40: 宽 / 高
///   - 偏移 64: imgSize，最低位为填充标志 pad，数据大小 = pad ? imgSize>>8 : imgSize
///   - 偏移 68+pad: 图像数据
fn decode_ktx_image(src: &Path) -> Result<bool, String> {
    let buf = fs::read(src).map_err(|e| e.to_string())?;
    if buf.len() < 68 || &buf[..12] != KTX_MAGIC {
        return Ok(false);
    }
    let ifmt = u32le(&buf, 28);
    let w = u32le(&buf, 36) as usize;
    let h = u32le(&buf, 40) as usize;
    let img_size = u32le(&buf, 64) as usize;
    let pad = img_size & 1;
    let data_size = if pad == 1 { img_size >> 8 } else { img_size };
    let start = 68 + pad;
    if buf.len() < start + data_size {
        return Err(format!("文件截断: {} 不足 {data_size} 字节数据", src.display()));
    }
    let data = &buf[start..start + data_size];

    let (rgba, color): (Vec<u8>, image::ExtendedColorType) = match ifmt {
        IFMT_BC7 | IFMT_DXT1 | IFMT_DXT3 | IFMT_DXT5 => (
            decode_bc(ifmt, data, w, h, src)?,
            image::ExtendedColorType::Rgba8,
        ),
        IFMT_RGBA8 => {
            if data.len() != w * h * 4 {
                return Err(format!("RGBA8 长度异常: {} != {}", data.len(), w * h * 4));
            }
            (data.to_vec(), image::ExtendedColorType::Rgba8)
        }
        IFMT_RGB8 => {
            if data.len() != w * h * 3 {
                return Err(format!("RGB8 长度异常: {} != {}", data.len(), w * h * 3));
            }
            (data.to_vec(), image::ExtendedColorType::Rgb8)
        }
        _ => return Err(format!("不支持的格式: 0x{ifmt:04x} ({})", src.display())),
    };

    // 就地还原为 PNG；原扩展名不是 .png 的，写 .png 并删除原文件
    let dst = src.with_extension("png");
    image::save_buffer(&dst, &rgba, w as u32, h as u32, color)
        .map_err(|e| format!("写 PNG 失败: {e}"))?;
    if dst != src {
        fs::remove_file(src).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

/// 扫描目录树，就地还原所有伪 KTX 图片。返回 (还原数, 失败数)。
fn decode_images_inplace(root: &Path) -> (usize, usize) {
    let mut ok = 0usize;
    let mut fail = 0usize;
    let mut files = Vec::new();
    walk(root, &mut files);
    files.sort();
    for f in files {
        let probe = fs::read(&f).unwrap_or_default();
        if probe.len() < 12 || &probe[..12] != KTX_MAGIC {
            continue;
        }
        match decode_ktx_image(&f) {
            Ok(_) => ok += 1,
            Err(e) => {
                fail += 1;
                println!("    [图片失败] {}: {e}", f.strip_prefix(root).unwrap().display());
            }
        }
    }
    (ok, fail)
}

// ---------------- 主流程 ----------------

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn restore(input_path: &Path, out_root: &Path, keep_temp: bool, decode_images: bool) {
    fs::create_dir_all(out_root).unwrap();
    let final_dir = out_root.join("files"); // 最终还原产物

    let tmpdir = out_root.join(format!("tnnd_{}", std::process::id()));
    fs::create_dir_all(&tmpdir).unwrap();
    let raw_dir = tmpdir.join("raw_7z"); // 7z 直接解出的内容（中间产物，默认随临时目录清理）
    let result = std::panic::catch_unwind(|| {
        // 1. TNND 解密（如有）
        let dec_7z;
        if is_tnnd(input_path) {
            dec_7z = tmpdir.join(format!(
                "{}.dec.7z",
                input_path.file_stem().unwrap().to_string_lossy()
            ));
            let n = tnnd_decrypt_file(input_path, &dec_7z).unwrap();
            println!("[1/4] TNND 解密: {} -> {n} 字节", input_path.display());
        } else {
            dec_7z = input_path.to_path_buf();
            println!("[1/4] 无 TNND 头，按明文 7z 处理: {}", input_path.display());
        }

        // 2. 解压 7z
        fs::create_dir_all(&raw_dir).unwrap();
        extract_7z(&dec_7z, &raw_dir);
        println!("[2/4] 7z 解压完成（中间目录）");

        // 3. 处理解出的每个文件：UPAK 解包 / TNND 解密 / 原样拷贝
        fs::create_dir_all(&final_dir).unwrap();
        let mut files = Vec::new();
        walk(&raw_dir, &mut files);
        files.sort();
        for f in files {
            let rel = f.strip_prefix(&raw_dir).unwrap().to_path_buf();
            let magic = fs::read(&f).unwrap_or_default();
            let magic4 = magic.get(..4).unwrap_or(&[]);
            if magic4 == UPAK_MAGIC {
                let target_dir = final_dir.join(rel.parent().unwrap()).join(f.file_stem().unwrap());
                let count = upak_extract(&f, &target_dir);
                println!("[3/4] UPAK 解包: {} -> {} ({count} 个文件)", rel.display(), target_dir.display());
            } else if magic4 == TNND_MAGIC.as_slice() {
                let target = final_dir.join(&rel);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                tnnd_decrypt_file(&f, &target).unwrap();
                println!("[3/4] TNND 解密: {} -> {}", rel.display(), target.display());
            } else {
                let target = final_dir.join(&rel);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::copy(&f, &target).unwrap();
                println!("[3/4] 明文拷贝: {}", rel.display());
            }
        }

        // 4. 图片还原：伪 KTX 就地解码为 PNG
        if decode_images {
            let (ok, fail) = decode_images_inplace(&final_dir);
            println!("[4/4] 图片还原: {ok} 张已解码为 PNG，{fail} 张失败");
        } else {
            println!("[4/4] 已按 --no-decode-images 跳过图片还原");
        }
    });

    if keep_temp && result.is_ok() {
        println!("临时文件保留于: {}", tmpdir.display());
    } else {
        let _ = fs::remove_dir_all(&tmpdir);
    }
    match result {
        Ok(()) => println!("完成。最终产物目录: {}", final_dir.display()),
        Err(e) => std::panic::resume_unwind(e),
    }
}

fn main() {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut keep_temp = false;
    let mut decode_images = true;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-o" | "--output" => {
                output = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                    eprintln!("-o/--output 缺少参数");
                    std::process::exit(1);
                })))
            }
            "--keep-temp" => keep_temp = true,
            "--no-decode-images" => decode_images = false,
            "-h" | "--help" => {
                println!("用法: restore_game <加密7z路径> [-o 输出目录] [--keep-temp] [--no-decode-images]");
                return;
            }
            _ if input.is_none() => input = Some(PathBuf::from(a)),
            _ => {
                eprintln!("未知参数: {a}");
                std::process::exit(1);
            }
        }
    }
    let input = input.unwrap_or_else(|| {
        eprintln!("用法: restore_game <加密7z路径> [-o 输出目录] [--keep-temp] [--no-decode-images]");
        std::process::exit(1);
    });
    if !input.is_file() {
        eprintln!("输入文件不存在: {}", input.display());
        std::process::exit(1);
    }
    let out = output.unwrap_or_else(|| {
        input.with_file_name(format!(
            "{}_restored",
            input.file_stem().unwrap().to_string_lossy()
        ))
    });
    restore(&input, &out, keep_temp, decode_images);
}
