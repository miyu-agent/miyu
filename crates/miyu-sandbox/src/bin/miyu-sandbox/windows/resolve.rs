//! 先认程序（施工 5-9 改上）：MXC 起不来的命令一律说成「收紧不成」，找不到、不是程序的就分不出来。照 `CreateProcess` 找程序的
//! 先后（当前目录，再 `PATH`），没写扩展名的照 `PATHEXT` 一个个试：找不到的照旧退 127，`.exe`、`.com` 开头不是 `MZ` 的退 126，
//! 同别的平台、同 5-9 以前（`run.rs` 的测试守着）。

use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// 没设 `PATHEXT` 时试的扩展名。
const PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// 程序 `program` 找得到、像个程序：交回找到的位置；找不到的 `NotFound`，不是程序的别的错。
pub(super) fn program(program: &OsStr) -> io::Result<PathBuf> {
    let path = Path::new(program);
    let roots: Vec<PathBuf> = if path.is_absolute() || path.components().count() > 1 {
        vec![PathBuf::new()]
    } else {
        std::env::current_dir()
            .into_iter()
            .chain(
                std::env::var_os("PATH")
                    .iter()
                    .flat_map(std::env::split_paths),
            )
            .collect()
    };
    let extensions: Vec<OsString> = match path.extension() {
        Some(_) => vec![OsString::new()],
        None => std::env::var("PATHEXT")
            .unwrap_or_else(|_| PATHEXT.to_string())
            .split(';')
            .filter(|ext| !ext.is_empty())
            .map(OsString::from)
            .collect(),
    };
    let found = roots
        .iter()
        .flat_map(|root| {
            extensions.iter().map(move |ext| {
                let mut name = root.join(path).into_os_string();
                name.push(ext);
                PathBuf::from(name)
            })
        })
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let binary = found
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("com"));
    if binary {
        let mut head = [0_u8; 2];
        let read = std::fs::File::open(&found).and_then(|mut file| file.read_exact(&mut head));
        if read.is_err() || &head != b"MZ" {
            return Err(io::Error::other("not a Windows program"));
        }
    }
    Ok(found)
}
