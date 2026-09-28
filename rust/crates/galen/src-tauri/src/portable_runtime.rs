//! Runtime payload for the single-file Windows build.
//!
//! Windows cannot execute an embedded child process directly from memory, so
//! the portable Galen executable expands its three command-line runtimes into
//! a versioned per-user cache on first launch. The user still receives and
//! starts one EXE; later launches reuse the verified files.

use include_flate::{flate, IFlate};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

flate!(static TYPST: IFlate from "binaries/typst-x86_64-pc-windows-msvc.exe" with zstd);
flate!(static DENO: IFlate from "binaries/deno-x86_64-pc-windows-msvc.exe" with zstd);
flate!(static UV: IFlate from "binaries/uv-x86_64-pc-windows-msvc.exe" with zstd);

const TYPST_SIZE: u64 = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/binaries/typst-x86_64-pc-windows-msvc.exe"
))
.len() as u64;
const DENO_SIZE: u64 = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/binaries/deno-x86_64-pc-windows-msvc.exe"
))
.len() as u64;
const UV_SIZE: u64 = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/binaries/uv-x86_64-pc-windows-msvc.exe"
))
.len() as u64;

pub fn runtime_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|root| {
        root.join("Galen")
            .join("runtime")
            .join(env!("CARGO_PKG_VERSION"))
    })
}

pub fn ensure() -> Result<PathBuf, String> {
    let dir = runtime_dir().ok_or_else(|| "无法定位 Windows 用户数据目录".to_string())?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("无法创建 Galen 便携运行时目录：{error}"))?;

    install(&dir, "typst.exe", &TYPST, TYPST_SIZE)?;
    install(&dir, "deno.exe", &DENO, DENO_SIZE)?;
    install(&dir, "uv.exe", &UV, UV_SIZE)?;
    Ok(dir)
}

fn install(dir: &Path, name: &str, payload: &IFlate, expected_size: u64) -> Result<(), String> {
    let target = dir.join(name);
    if fs::metadata(&target)
        .map(|metadata| metadata.len() == expected_size)
        .unwrap_or(false)
    {
        return Ok(());
    }

    let temporary = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let decoded = payload.decoded();
    if decoded.len() as u64 != expected_size {
        return Err(format!("内置运行时 {name} 校验失败"));
    }

    let mut file = fs::File::create(&temporary)
        .map_err(|error| format!("无法释放 {name}：{error}"))?;
    file.write_all(&decoded)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("无法写入 {name}：{error}"))?;
    drop(file);
    drop(decoded);

    if target.exists() {
        fs::remove_file(&target).map_err(|error| format!("无法更新 {name}：{error}"))?;
    }
    fs::rename(&temporary, &target).map_err(|error| format!("无法启用 {name}：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_payload_sizes_match_sources() {
        assert_eq!(TYPST.decoded().len() as u64, TYPST_SIZE);
        assert_eq!(DENO.decoded().len() as u64, DENO_SIZE);
        assert_eq!(UV.decoded().len() as u64, UV_SIZE);
    }
}
