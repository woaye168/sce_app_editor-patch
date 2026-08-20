#!/usr/bin/env python3
"""SCE 加密包一键还原工具：TNND 解密 → 7z 解压 → UPAK 解包 → 图片(KTX)还原。

用法：
    python restore_game.py <加密7z路径> [-o 输出目录] [--keep-temp] [--no-decode-images]

示例：
    python restore_game.py windows_game.7z
    python restore_game.py windows_game.7z -o restored

流程说明：
    1. 输入文件若以 TNND 开头，先按 CREATEEASY 循环 XOR 解密为 7z；
       无 TNND 头则直接当 7z 处理。
    2. 解压 7z（优先 py7zr，其次系统 7z.exe / tar）。
    3. 对其中的 .pak（UPAK 格式）逐条目解出完整文件树；
       其它文件若是 TNND 加密则顺带解密，否则原样拷贝。
    4. 扫描产物中的伪 KTX 图片（魔数 \\xABKTX 11\\xBB，BC7/RGBA8/RGB8），
       解码并就地还原为真正的 PNG。依赖 Pillow + texture2ddecoder，
       缺依赖时警告并跳过（或用 --no-decode-images 主动跳过）。

只读研究工具：不修改输入文件。
"""

import argparse
import mmap
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

TNND_MAGIC = b"TNND"
TNND_KEY = b"CREATEEASY"
UPAK_MAGIC = b"UPAK"
CHUNK = 4 * 1024 * 1024

# 伪 KTX 纹理（.png/.tga 扩展名的加密图片）
KTX_MAGIC = b"\xabKTX 11\xbb\r\n\x1a\n"
IFMT_BC7 = 0x8E8C    # GL_COMPRESSED_RGBA_BPTC_UNORM
IFMT_RGBA8 = 0x8058  # GL_RGBA8
IFMT_RGB8 = 0x8051   # GL_RGB8
IFMT_DXT1 = 0x83F1   # GL_COMPRESSED_RGBA_S3TC_DXT1_EXT (BC1)
IFMT_DXT3 = 0x83F2   # GL_COMPRESSED_RGBA_S3TC_DXT3_EXT (BC2)
IFMT_DXT5 = 0x83F3   # GL_COMPRESSED_RGBA_S3TC_DXT5_EXT (BC3)


# ---------------- TNND ----------------

def is_tnnd(path: Path) -> bool:
    with open(path, "rb") as f:
        return f.read(4) == TNND_MAGIC


def tnnd_decrypt_file(src: Path, dst: Path) -> int:
    """TNND 解密（流式 XOR）。返回写出字节数。调用方需先确认 is_tnnd。"""
    key = TNND_KEY
    klen = len(key)
    written = 0
    with open(src, "rb") as fin, open(dst, "wb") as fout:
        head = fin.read(4)
        if head != TNND_MAGIC:
            raise ValueError(f"不是 TNND 文件: {src}")
        pos = 0
        while True:
            chunk = fin.read(CHUNK)
            if not chunk:
                break
            buf = bytearray(chunk)
            for i in range(len(buf)):
                buf[i] ^= key[(pos + i) % klen]
            fout.write(buf)
            pos += len(buf)
            written += len(buf)
    return written


# ---------------- 7z ----------------

def extract_7z(archive: Path, outdir: Path) -> None:
    """解压 7z：py7zr → 7z 可执行文件 → Windows 自带 tar。"""
    try:
        import py7zr  # type: ignore
        with py7zr.SevenZipFile(archive, "r") as z:
            z.extractall(outdir)
        return
    except ImportError:
        pass

    candidates = [shutil.which("7z"), shutil.which("7za")]
    candidates += [
        r"C:\Program Files\7-Zip\7z.exe",
        r"C:\Program Files (x86)\7-Zip\7z.exe",
    ]
    for exe in candidates:
        if exe and Path(exe).exists():
            subprocess.run([exe, "x", str(archive), f"-o{outdir}", "-y"],
                           check=True)
            return

    tar = shutil.which("tar")
    if tar:  # Windows 10+ 自带 bsdtar，支持 7z
        subprocess.run([tar, "-xf", str(archive), "-C", str(outdir)],
                       check=True)
        return

    raise RuntimeError("找不到可用的 7z 解压方式：请 pip install py7zr 或安装 7-Zip")


# ---------------- UPAK ----------------

def _safe_rel(name: str) -> Path:
    """防路径穿越：去掉盘符/绝对路径，.. 替换为 __。"""
    name = name.replace("\\", "/").lstrip("/")
    parts = ["__" if p == ".." else p for p in name.split("/") if p not in ("", ".")]
    if parts and ":" in parts[0]:
        parts[0] = parts[0].replace(":", "_")
    return Path(*parts) if parts else Path("_unnamed")


def upak_extract(pak_path: Path, outdir: Path) -> int:
    """解包 SCE UPAK（条目 = 名字\\0 + u32 offset + u32 size + u32 checksum）。返回条目数。"""
    with open(pak_path, "rb") as f:
        data = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)

    with data:
        if data[:4] != UPAK_MAGIC:
            raise ValueError(f"不是 UPAK 文件: {pak_path}")
        (count,) = struct.unpack_from("<I", data, 4)
        # 偏移 8 为 u32 总校验，索引区从 12 开始
        p = 12
        ok = 0
        for _ in range(count):
            end = data.find(b"\x00", p)
            if end < 0:
                raise ValueError("条目名缺少 \\0 结尾，索引损坏")
            name = data[p:end].decode("utf-8", errors="replace")
            p = end + 1
            offset, size, _checksum = struct.unpack_from("<III", data, p)
            p += 12  # offset + size + checksum（比标准 Urho3D 多 4 字节校验）

            target = outdir / _safe_rel(name)
            target.parent.mkdir(parents=True, exist_ok=True)
            with open(target, "wb") as out:
                out.write(data[offset:offset + size])
            ok += 1
    return ok


# ---------------- 图片还原（伪 KTX → PNG） ----------------

def decode_ktx_image(src: Path) -> bool:
    """解码单个伪 KTX 文件并就地保存为 PNG。非 KTX 文件返回 False。

    格式（伪装成 KTX 纹理）：
      - 12 字节魔数: AB 4B 54 58 20 31 31 BB 0D 0A 1A 0A
      - 偏移 28: glInternalFormat（0x8E8C=BC7, 0x8058=RGBA8, 0x8051=RGB8,
                  0x83F1=DXT1/BC1, 0x83F2=DXT3/BC2, 0x83F3=DXT5/BC3）
      - 偏移 36/40: 宽 / 高
      - 偏移 64: imgSize，最低位为填充标志 pad，数据大小 = pad ? imgSize>>8 : imgSize
      - 偏移 68+pad: 图像数据（BC 压缩格式 R/B 通道互换，RGBA8/RGB8 不互换）
    """
    import texture2ddecoder  # type: ignore
    from PIL import Image    # type: ignore

    buf = src.read_bytes()
    if buf[:12] != KTX_MAGIC:
        return False

    (ifmt,) = struct.unpack_from("<I", buf, 28)
    w, h = struct.unpack_from("<II", buf, 36)
    (img_size,) = struct.unpack_from("<I", buf, 64)
    pad = img_size & 1
    data_size = (img_size >> 8) if pad else img_size
    data = buf[68 + pad: 68 + pad + data_size]

    # BC 压缩格式 -> (解码函数, 期望数据字节数)；长度不符时拒绝调用
    # （texture2ddecoder 原生层不做边界检查，传错长度会直接段错误；
    #   库不支持 BC2/DXT3，遇到会走下方"不支持的格式"报错）
    bc_decoders = {
        IFMT_BC7: (texture2ddecoder.decode_bc7, w * h),
        IFMT_DXT1: (texture2ddecoder.decode_bc1, w * h // 2),
        IFMT_DXT5: (texture2ddecoder.decode_bc3, w * h),
    }

    if ifmt in bc_decoders:
        decoder, expected = bc_decoders[ifmt]
        if len(data) != expected:
            raise ValueError(
                f"数据长度异常: {len(data)} != {expected} (0x{ifmt:04x}, {w}x{h}, {src})")
        img = Image.frombytes("RGBA", (w, h), decoder(data, w, h))
        r, g, b, a = img.split()
        img = Image.merge("RGBA", (b, g, r, a))  # 原生解码输出 BGRA，需 R/B 互换
    elif ifmt == IFMT_RGBA8:
        img = Image.frombytes("RGBA", (w, h), data)
    elif ifmt == IFMT_RGB8:
        img = Image.frombytes("RGB", (w, h), data)
    else:
        raise ValueError(f"不支持的格式: 0x{ifmt:04x} ({src})")

    # 就地还原为 PNG；原扩展名不是 .png 的，写 .png 并删除原文件
    dst = src.with_suffix(".png")
    img.save(dst)
    if dst != src:
        src.unlink()
    return True


def decode_images_inplace(root: Path) -> tuple[int, int]:
    """扫描目录树，就地还原所有伪 KTX 图片。返回 (还原数, 失败数)。"""
    ok, fail = 0, 0
    for f in sorted(root.rglob("*")):
        if not f.is_file():
            continue
        with open(f, "rb") as probe:
            if probe.read(12) != KTX_MAGIC:
                continue
        try:
            decode_ktx_image(f)
            ok += 1
        except Exception as e:
            fail += 1
            print(f"    [图片失败] {f.relative_to(root)}: {e}")
    return ok, fail


# ---------------- 主流程 ----------------

def restore(input_path: Path, out_root: Path, keep_temp: bool = False,
            decode_images: bool = True) -> None:
    out_root.mkdir(parents=True, exist_ok=True)
    final_dir = out_root / "files"   # 最终还原产物

    tmpdir = Path(tempfile.mkdtemp(prefix="tnnd_", dir=out_root))
    raw_dir = tmpdir / "raw_7z"      # 7z 直接解出的内容（中间产物，默认随临时目录清理）
    try:
        # 1. TNND 解密（如有）
        if is_tnnd(input_path):
            dec_7z = tmpdir / (input_path.stem + ".dec.7z")
            n = tnnd_decrypt_file(input_path, dec_7z)
            print(f"[1/4] TNND 解密: {input_path.name} -> {n} 字节")
        else:
            dec_7z = input_path
            print(f"[1/4] 无 TNND 头，按明文 7z 处理: {input_path.name}")

        # 2. 解压 7z
        raw_dir.mkdir(parents=True, exist_ok=True)
        extract_7z(dec_7z, raw_dir)
        print("[2/4] 7z 解压完成（中间目录）")

        # 3. 处理解出的每个文件：UPAK 解包 / TNND 解密 / 原样拷贝
        final_dir.mkdir(parents=True, exist_ok=True)
        for f in sorted(raw_dir.rglob("*")):
            if not f.is_file():
                continue
            rel = f.relative_to(raw_dir)
            with open(f, "rb") as probe:
                magic = probe.read(4)
            if magic == UPAK_MAGIC:
                target_dir = final_dir / rel.parent / f.stem
                count = upak_extract(f, target_dir)
                print(f"[3/4] UPAK 解包: {rel} -> {target_dir} ({count} 个文件)")
            elif magic == TNND_MAGIC:
                target = final_dir / rel
                target.parent.mkdir(parents=True, exist_ok=True)
                tnnd_decrypt_file(f, target)
                print(f"[3/4] TNND 解密: {rel} -> {target}")
            else:
                target = final_dir / rel
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(f, target)
                print(f"[3/4] 明文拷贝: {rel}")

        # 4. 图片还原：伪 KTX 就地解码为 PNG
        if decode_images:
            try:
                import texture2ddecoder  # noqa: F401
                import PIL               # noqa: F401
            except ImportError:
                print("[4/4] 跳过图片还原：缺少依赖，请 pip install Pillow texture2ddecoder")
            else:
                ok, fail = decode_images_inplace(final_dir)
                print(f"[4/4] 图片还原: {ok} 张已解码为 PNG，{fail} 张失败")
        else:
            print("[4/4] 已按 --no-decode-images 跳过图片还原")

        if keep_temp:
            print(f"临时文件保留于: {tmpdir}")
        else:
            shutil.rmtree(tmpdir, ignore_errors=True)
        print(f"完成。最终产物目录: {final_dir}")
    except Exception:
        shutil.rmtree(tmpdir, ignore_errors=True)
        raise


def main() -> None:
    ap = argparse.ArgumentParser(
        description="SCE 加密包一键还原：TNND 解密 → 7z 解压 → UPAK 解包 → 图片还原")
    ap.add_argument("input", type=Path, help="加密的 7z 文件路径（TNND 头）")
    ap.add_argument("-o", "--output", type=Path, default=None,
                    help="输出目录（默认 <输入名>_restored）")
    ap.add_argument("--keep-temp", action="store_true",
                    help="保留中间产物（解密后的 7z）")
    ap.add_argument("--no-decode-images", action="store_true",
                    help="跳过图片还原（伪 KTX → PNG）")
    args = ap.parse_args()

    if not args.input.is_file():
        sys.exit(f"输入文件不存在: {args.input}")
    out = args.output or args.input.with_name(args.input.stem + "_restored")
    restore(args.input, out, keep_temp=args.keep_temp,
            decode_images=not args.no_decode_images)


if __name__ == "__main__":
    main()
