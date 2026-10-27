//! 要问人的那几条（施工 D-1，从 `guard.rs` 挪出来，施工 X-10 上那边到了行数上限）：提的放行规则、给头看的说明，放行过的
//! 规则管不管得到。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_fs::{Boundary, Zone, resolve};
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::tool::Access;

use super::{module, raw};

/// 要问人的一条路径：真实的位置、是不是写、在哪一片；放行的范围是不是只放这一条本身（[`Asked::alone`]）。
pub(super) struct Asked {
    pub(super) real: PathBuf,
    pub(super) write: bool,
    pub(super) zone: Zone,
    pub(super) alone: bool,
}

impl Asked {
    /// 照边界表 `boundary` 记下要问人的 `real`。受保护的路径（施工 5-17 下）是文件、它的上级又不受保护的（工作区里的
    /// `.mcp.json`），放行的范围只放这一条本身：放它的上级盖不住它（[`granted`]），点了「本会话都允许」还得再问。
    pub(super) fn new(real: PathBuf, write: bool, zone: Zone, boundary: &Boundary) -> Asked {
        let alone = boundary.protected(&real)
            && !real.is_dir()
            && real
                .parent()
                .is_some_and(|parent| !boundary.protected(parent));
        Asked {
            real,
            write,
            zone,
            alone,
        }
    }
}

/// 要问人：提的放行规则列出放行的范围（`scope`），给头看的说明列出每一条。
pub(super) fn ask(name: &str, access: Access, asked: &[Asked], home: Option<&Path>) -> Verdict {
    let dirs = |write: bool| -> Vec<String> {
        let mut dirs: Vec<String> = asked
            .iter()
            .filter(|asked| asked.write == write)
            .map(|asked| text(&scope(asked, home)))
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
/// 是家目录本身、或者更上面的，只放这一条本身：家目录里有钥匙（施工 D-1）；受保护的文件上级不受保护的也只放这一条本身（施工
/// 5-17 下，[`Asked::alone`]）。`real`、`home` 都是真实的位置。
fn scope(asked: &Asked, home: Option<&Path>) -> PathBuf {
    let real = &asked.real;
    if asked.alone {
        return real.clone();
    }
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

/// 认出了高危的（施工 5-17）：原来就拒的照拒；灭顶的拒，写给她工具交的那一句；别的问人，不提规则（高危的不该一劳永逸），说明
/// 照不报路径的那种（第五条第 4 款）再多 `hazard`。
pub(super) fn hazardous(
    verdict: Verdict,
    (name, access): (&str, Access),
    hazard: miyu_tool::Hazard,
    asking: &[(&'static str, String)],
) -> Verdict {
    if matches!(verdict, Verdict::Deny { .. }) {
        return verdict;
    }
    if let Some(worded) = hazard.refused {
        return super::deny(worded);
    }
    let mut detail: Value = serde_json::from_str(super::described(name, &access, asking).get())
        .unwrap_or_else(|_| json!({ "tool": name }));
    detail["hazard"] = json!(hazard.code);
    Verdict::Ask {
        module: module(),
        access,
        rule: None,
        detail: Some(raw(&detail)),
    }
}

/// 工作目录 `cwd` 的真实位置；换不了的照写的。
pub(super) fn real_dir(cwd: &str, home: Option<&Path>) -> PathBuf {
    resolve(Path::new("/"), home, cwd).unwrap_or_else(|_| PathBuf::from(cwd))
}

/// 命令越过沙盒问人时提的规则（施工 5-15，「此目录不再询问」）：放行工作目录 `cwd`（真实的位置）的写，连同下面的。
/// 家目录本身也照放：人看得到放行的是哪个目录，私密目录照旧另问（[`granted`]），沙盒照旧藏数据根。工具报了命令前缀的另写
/// `prefix`（施工 SB-3）：选了的，这个目录下同一前缀的命令越过沙盒不再问（[`prefixed`]）。
pub(super) fn this_dir(name: &str, cwd: &Path, prefix: Option<&str>) -> RawJson {
    let mut rule = json!({ "tool": name, "write": [text(cwd)] });
    if let Some(prefix) = prefix {
        rule["prefix"] = json!(prefix);
    }
    raw(&rule)
}

/// 放行过的规则里有一条认这一次越过沙盒的命令（施工 SB-3）：工具 `name` 一样，前缀 `prefix` 一字不差，工作目录的真实位置
/// `cwd` 是规则放行写的某个目录本身、或者在它下面（按路径的段比，同 [`granted`]）。读不懂的、没写前缀的规则不算。
pub(super) fn prefixed(grants: &[RawJson], name: &str, prefix: &str, cwd: &Path) -> bool {
    grants.iter().any(|rule| {
        serde_json::from_str::<Value>(rule.get())
            .ok()
            .is_some_and(|rule| {
                rule.get("tool").and_then(Value::as_str) == Some(name)
                    && rule.get("prefix").and_then(Value::as_str) == Some(prefix)
                    && rule
                        .get("write")
                        .and_then(Value::as_array)
                        .is_some_and(|scopes| {
                            scopes
                                .iter()
                                .filter_map(Value::as_str)
                                .any(|scope| cwd.starts_with(scope))
                        })
            })
    })
}

/// 要问人的一条落在放行过的规则里没有（施工 D-1）：规则里照是读是写（`read`、`write`）列着放行的范围，这一条是其中一个
/// 本身、或者在它下面。按路径的段比（`Path::starts_with`），`/a/b` 不放行 `/a/bc`。读不懂的规则不算。私密目录里的那一条只认
/// 放行范围本身也在私密目录里的规则（施工 5-15）：放行了家目录不等于放行了 `~/.ssh`。受保护的路径同理（施工 5-17 下，2026-10-22
/// 定，同 Claude Code 的危险文件）：对整个工作区点了「此目录不再询问」，`.git/hooks` 照旧问。
pub(super) fn granted(grants: &[RawJson], asked: &Asked, boundary: &Boundary) -> bool {
    let key = if asked.write { "write" } else { "read" };
    let covers = |scope: &str| {
        asked.real.starts_with(scope)
            && (asked.zone != Zone::Private || boundary.zone(Path::new(scope)) == Zone::Private)
            && (!boundary.protected(&asked.real) || boundary.protected(Path::new(scope)))
    };
    grants.iter().any(|rule| {
        serde_json::from_str::<Value>(rule.get())
            .ok()
            .and_then(|rule| rule.get(key).and_then(Value::as_array).cloned())
            .is_some_and(|scopes| scopes.iter().filter_map(Value::as_str).any(covers))
    })
}

/// 路径写成字。
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
