//! 规则集的指纹（施工 V-47）：从头建一次要看的每一样，除了「逐条打开、加规则」那一步，都照样再看一遍、记成字节。两次的
//! 字节一样，此刻从头建会加的规则就和建缓存时加的一条不差、每条落在同一个文件上（理由见
//! `docs/construction/V-47-沙盒规则集缓存.md`「怎么认还能不能用」）。
//!
//! 记的是：本进程的有效用户、组；读到的挂载表原样；换成真实位置、加上别名以后的藏起来的；一级级走进的每个目录的路径、它里面
//! 每一样的名字和怎么放的（藏起来的、链接、走进去、放读，照列出来的先后）；每一条规则的路径此刻是什么文件（设备号、inode 号、
//! 种类，或者查不了的错误号）。不看时间戳。
//!
//! 「是什么文件」怎么查：一级级放读的那些，用列目录时那个目录的描述符 `fstatat(…, AT_SYMLINK_NOFOLLOW | AT_NO_AUTOMOUNT)`（只查
//! 这一段，不从根目录再解析一遍；不碰自动挂载点，和建规则时 `O_PATH` 打开一样不触发挂载）；别的几条照路径 `fstatat(…,
//! AT_NO_AUTOMOUNT)`（跟链接，和 `O_PATH` 打开一样）。
//!
//! 以后从头建多看了什么（`reads.rs` 的走法、`rules.rs` 排规则时多照什么），这里要跟着记上，不然缓存认不出那一样变了。

use std::ffi::OsStr;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, CWD, Mode, OFlags};

use crate::Spec;

use super::reads::{Seen, Step, plan_seen};
use super::rules::{self, Ident, Layer, Rule};

/// 一份指纹：字节（比的就是它），和照它排出来的缓存那一层的规则、每条事先查到的是什么文件（建的时候照它们核对）。
#[derive(Debug)]
pub(crate) struct Print {
    /// 记成的字节。
    pub(crate) bytes: Vec<u8>,
    /// 缓存那一层（[`Layer::Base`]）的规则。
    pub(crate) rules: Vec<Rule>,
    /// 每条规则的路径此刻是什么文件，和 `rules` 一一对应。
    pub(crate) idents: Vec<Ident>,
}

/// 照规格 `spec` 记一份指纹。规格要能缓存（没写 `mount`、`read`，路径都是绝对的），调的一方先查。
///
/// # Errors
///
/// 从头建会报错的（一级级走时目录列不出来之类），或者走进的目录打不开：说原话。调的一方不缓存，交给助手从头建，报的错照旧。
pub(crate) fn print(spec: &Spec) -> Result<Print, String> {
    let mut trace = Trace::default();
    trace.u32(rustix::process::geteuid().as_raw());
    trace.u32(rustix::process::getegid().as_raw());
    let plan = plan_seen(&spec.hidden, &mut trace)?;
    if let Some(error) = trace.failed.take() {
        return Err(error);
    }
    let Trace {
        mut bytes,
        allowed,
        walked,
        ..
    } = trace;
    if walked && allowed.len() != plan.len() {
        return Err("walk and plan disagree".to_string());
    }
    let count = plan.len();
    let rules = rules::base(spec, plan, Layer::Base);
    let mut allowed = allowed.into_iter();
    let idents: Vec<Ident> = rules
        .iter()
        .enumerate()
        .map(|(at, rule)| {
            // 第 0 条是根目录；接着 `count` 条是 `plan` 的，一级级走过的用走时查到的。
            let planned = walked && (1..=count).contains(&at);
            match planned.then(|| allowed.next()).flatten() {
                Some(ident) => ident,
                None => follow(&rule.path),
            }
        })
        .collect();
    bytes.push(b'I');
    for ident in &idents {
        ident.write(&mut bytes);
    }
    Ok(Print {
        bytes,
        rules,
        idents,
    })
}

/// 照路径查是什么文件（跟链接，不触发自动挂载），和建规则时 `O_PATH` 打开再 `fstat` 查到的一样。
fn follow(path: &Path) -> Ident {
    Ident::of(rustix::fs::statat(CWD, path, AtFlags::NO_AUTOMOUNT))
}

/// 一级级走时记的（[`Seen`]）。
#[derive(Default)]
struct Trace {
    bytes: Vec<u8>,
    /// 走进的目录，一层一个描述符：查这一级放读的那几样用。
    dirs: Vec<OwnedFd>,
    /// 放读的每一样此刻是什么文件，照放的先后。
    allowed: Vec<Ident>,
    /// 一级级走过（藏起来的都不在的，`plan` 直接交回根目录，不走）。
    walked: bool,
    /// 走进的目录打不开：不缓存。
    failed: Option<String>,
}

impl Trace {
    fn u32(&mut self, value: u32) {
        self.bytes.extend(value.to_le_bytes());
    }

    /// 一段字节，前面写长度，几段连着也认不混。
    fn chunk(&mut self, bytes: &[u8]) {
        self.bytes
            .extend(u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
        self.bytes.extend(bytes);
    }
}

impl Seen for Trace {
    fn table(&mut self, table: Option<&[u8]>) {
        match table {
            Some(table) => {
                self.bytes.push(b'M');
                self.chunk(table);
            }
            None => self.bytes.push(b'm'),
        }
    }

    fn real(&mut self, real: &[PathBuf]) {
        self.bytes.push(b'R');
        self.chunk(&u32::try_from(real.len()).unwrap_or(u32::MAX).to_le_bytes());
        for path in real {
            self.chunk(path.as_os_str().as_bytes());
        }
    }

    fn enter(&mut self, dir: &Path) {
        self.walked = true;
        self.bytes.push(b'D');
        self.chunk(dir.as_os_str().as_bytes());
        let flags = OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC;
        match rustix::fs::open(dir, flags, Mode::empty()) {
            Ok(fd) => self.dirs.push(fd),
            Err(error) => {
                if self.failed.is_none() {
                    self.failed = Some(format!("cannot open {}: {error}", dir.display()));
                }
            }
        }
    }

    fn entry(&mut self, name: &OsStr, step: Step) {
        self.bytes.push(match step {
            Step::Hidden => b'h',
            Step::Link => b'l',
            Step::Down => b'd',
            Step::Allow => b'a',
        });
        self.chunk(name.as_bytes());
        if step == Step::Allow {
            let flags = AtFlags::SYMLINK_NOFOLLOW | AtFlags::NO_AUTOMOUNT;
            let ident = match self.dirs.last() {
                Some(dir) => Ident::of(rustix::fs::statat(dir, name, flags)),
                // 目录打不开的已经记在 `failed`，整份不用。
                None => Ident::Error(0),
            };
            self.allowed.push(ident);
        }
    }

    fn leave(&mut self) {
        self.bytes.push(b'U');
        self.dirs.pop();
    }
}
