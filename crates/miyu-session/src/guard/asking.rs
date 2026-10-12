//! 要问人的那几条（施工 D-1，从 `guard.rs` 挪出来，施工 X-10 上那边到了行数上限）：提的放行规则、给头看的说明，放行过的
//! 规则管不管得到。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_fs::Zone;
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::tool::Access;

use super::{module, raw};

/// 要问人的一条路径：真实的位置、是不是写、在哪一片。
pub(super) struct Asked {
    pub(super) real: PathBuf,
    pub(super) write: bool,
    pub(super) zone: Zone,
}

/// 要问人：提的放行规则列出放行的范围（`scope`），给头看的说明列出每一条。
pub(super) fn ask(name: &str, access: Access, asked: &[Asked], home: Option<&Path>) -> Verdict {
    let dirs = |write: bool| -> Vec<String> {
        let mut dirs: Vec<String> = asked
            .iter()
            .filter(|asked| asked.write == write)
            .map(|asked| text(&scope(&asked.real, home)))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    };
    let mut rule = json!({ "tool": name });
    for (key, write) in [("read", false), ("write", true)] {
        let listed = dirs(write);
        if !listed.is_empty() {
            rule[key] = json!(listed);
        }
    }
    let paths: Vec<Value> = asked
        .iter()
        .map(|asked| {
            json!({
                "path": text(&asked.real),
                "write": asked.write,
                "zone": match asked.zone {
                    Zone::Outside => "outside",
                    Zone::Private => "private",
                    _ => "read_only",
                },
            })
        })
        .collect();
    Verdict::Ask {
        module: module(),
        access,
        rule: Some(raw(&rule)),
        detail: Some(raw(&json!({ "tool": name, "paths": paths }))),
    }
}

/// 放行的范围（`11-权限与沙盒.md` 第二节「放行规则管多大」）：是目录的就是它自己，别的是它所在的目录，连同下面的；那个目录
/// 是家目录本身、或者更上面的，只放这一条本身：家目录里有钥匙（施工 D-1）。`real`、`home` 都是真实的位置。
fn scope(real: &Path, home: Option<&Path>) -> PathBuf {
    let directory = if real.is_dir() {
        real.to_path_buf()
    } else {
        real.parent()
            .map_or_else(|| real.to_path_buf(), Path::to_path_buf)
    };
    match home {
        Some(home) if home.starts_with(&directory) => real.to_path_buf(),
        _ => directory,
    }
}

/// 要问人的一条落在放行过的规则里没有（施工 D-1）：规则里照是读是写（`read`、`write`）列着放行的范围，这一条是其中一个
/// 本身、或者在它下面。按路径的段比（`Path::starts_with`），`/a/b` 不放行 `/a/bc`。读不懂的规则不算。
pub(super) fn granted(grants: &[RawJson], asked: &Asked) -> bool {
    let key = if asked.write { "write" } else { "read" };
    grants.iter().any(|rule| {
        serde_json::from_str::<Value>(rule.get())
            .ok()
            .and_then(|rule| rule.get(key).and_then(Value::as_array).cloned())
            .is_some_and(|scopes| {
                scopes
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|scope| asked.real.starts_with(scope))
            })
    })
}

/// 路径写成字。
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
