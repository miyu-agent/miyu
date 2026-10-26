//! 藏起来的另有挂载别名（施工 SEC-3，外部审计 M-1，`docs/blueprint/sandbox/linux.md`「怎么走」第 1 条）：同一片文件挂在了
//! 两处（bind mount、btrfs 的顶层子卷另挂一处），Landlock 的规则跟着目录本身走、不认路径，一级级放行把别名那一处整个放了，
//! 藏起来的从原路径也读得到；bubblewrap 的 tmpfs 只盖原路径，别名那一处照样看得到。照 `/proc/self/mountinfo` 找出每一条
//! 藏起来的在别的挂载里的位置，和它是同一个文件（设备号、inode 号都对上）的一起藏。

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// `/proc/self/mountinfo` 的一行里用得上的：挂的是哪个文件系统（`主:次` 设备号）、它里面的哪一片、挂在哪。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mount {
    dev: Vec<u8>,
    root: PathBuf,
    point: PathBuf,
}

/// `hidden`（真实的位置）连同它们的别名：每一条后面跟着它在别的挂载里的位置。读不了挂载表的照原样交回。
pub(super) fn with_aliases(hidden: &[PathBuf]) -> Vec<PathBuf> {
    let Ok(table) = fs::read("/proc/self/mountinfo") else {
        return hidden.to_vec();
    };
    with_aliases_in(hidden, &parse(&table))
}

/// 同 [`with_aliases`]，挂载表照 `mounts`。
fn with_aliases_in(hidden: &[PathBuf], mounts: &[Mount]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in hidden {
        out.push(path.clone());
        for (alias, original) in candidates(path, mounts) {
            if same_file(&alias, &original) && !out.contains(&alias) {
                out.push(alias);
            }
        }
    }
    out
}

/// 两条路径是同一个文件：设备号、inode 号都对上（跟链接，两边都是真实的位置）。看不了的不算。
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => (a.dev(), a.ino()) == (b.dev(), b.ino()),
        _ => false,
    }
}

/// 读挂载表：每行第 3、4、5 格是设备号、文件系统里的那一片、挂在哪；路径里的空白、反斜杠写成八进制转义。读不懂的行跳过。
fn parse(table: &[u8]) -> Vec<Mount> {
    table
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let mut fields = line.split(|byte| *byte == b' ').skip(2);
            let (dev, root, point) = (fields.next()?, fields.next()?, fields.next()?);
            Some(Mount {
                dev: dev.to_vec(),
                root: path(root),
                point: path(point),
            })
        })
        .collect()
}

/// 挂载表里的一格换成路径：`\040`（空格）、`\011`（制表）、`\012`（换行）、`\134`（反斜杠）换回原样，别的字节照原样。
fn path(field: &[u8]) -> PathBuf {
    let mut out = Vec::with_capacity(field.len());
    let mut at = 0;
    while let Some(&byte) = field.get(at) {
        let code = field
            .get(at + 1..at + 4)
            .filter(|_| byte == b'\\')
            .and_then(|code| std::str::from_utf8(code).ok())
            .and_then(|code| u8::from_str_radix(code, 8).ok());
        match code {
            Some(decoded) => {
                out.push(decoded);
                at += 4;
            }
            None => {
                out.push(byte);
                at += 1;
            }
        }
    }
    PathBuf::from(OsString::from_vec(out))
}

/// `path` 可能的别名，各带一条该和它是同一个文件的原路径：先找 `path` 在哪个挂载里（挂载点最长的那一个，同一处叠着挂的
/// 认最后挂的），换成文件系统里的位置；同一个文件系统的别的挂载里，那一片包着它的，交回它在那里的路径（对着 `path`）；那一片
/// 落在它里面的（把藏起来的里面一块挂到了别处），交回那个挂载点（对着 `path` 里面的那一块）。和原路径一样的不交。
fn candidates(path: &Path, mounts: &[Mount]) -> Vec<(PathBuf, PathBuf)> {
    let Some(home) = mounts
        .iter()
        .filter(|mount| path.starts_with(&mount.point))
        .max_by_key(|mount| mount.point.components().count())
    else {
        return Vec::new();
    };
    let Ok(inside) = path.strip_prefix(&home.point) else {
        return Vec::new();
    };
    let real = home.root.join(inside);
    let mut out: Vec<(PathBuf, PathBuf)> = Vec::new();
    for mount in mounts.iter().filter(|mount| mount.dev == home.dev) {
        let pair = if let Ok(rest) = real.strip_prefix(&mount.root) {
            (mount.point.join(rest), path.to_path_buf())
        } else if let Ok(rest) = mount.root.strip_prefix(&real) {
            (mount.point.clone(), path.join(rest))
        } else {
            continue;
        };
        if pair.0 != pair.1 && !out.contains(&pair) {
            out.push(pair);
        }
    }
    out
}

#[cfg(test)]
mod tests;
