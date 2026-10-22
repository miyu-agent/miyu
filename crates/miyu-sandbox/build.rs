//! 构建 id（施工 5-16 再补）：照这个包自己的源码（`src/` 下的文件、`Cargo.toml`、这个文件）算一个 64 位的 FNV-1a 摘要，
//! 写成十六位十六进制交给代码里的 `env!("MIYU_SANDBOX_BUILD")`。核心链进来的这个包和助手是同一份源码编的才一样：网页的
//! demo 树只编了 `-p miyu`，助手停在旧版，规格多了一格它认不出，所有沙盒命令报 bad spec 退出 125。

use std::path::{Path, PathBuf};

/// FNV-1a 的起始值和乘数（64 位）。
const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo 给了包的目录"));
    let mut files = vec![root.join("Cargo.toml"), root.join("build.rs")];
    collect(&root.join("src"), &mut files);
    files.sort();
    let mut hash = OFFSET;
    for file in &files {
        // 相对路径照 `/` 写：同一份源码在哪个目录、哪个系统上编都一样。
        let name = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(file).expect("读得了源码");
        for byte in name.bytes().chain([0]).chain(bytes).chain([0]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    println!("cargo:rustc-env=MIYU_SANDBOX_BUILD={hash:016x}");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
}

/// `dir` 下所有的文件，一层层往下找。
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("读得了源码目录") {
        let path = entry.expect("读得了源码目录").path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}
