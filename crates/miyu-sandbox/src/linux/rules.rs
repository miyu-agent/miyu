//! Landlock 的规则（`docs/blueprint/sandbox/linux.md`）：建规则集、照规格排出要加的规则、加上、收紧自己。施工 V-47 从助手
//! （原来的 `src/bin/miyu-sandbox/linux/landlock.rs`）挪进库里：助手每条命令从头建的、核心按规格缓存的，用的是同一份。
//!
//! 第 2 版的全部文件操作必须管得住：第 2 版起才管得了跨目录的改名、链接，更老的一律不许，很多工具会坏。更新的
//! 版本多出来的（截断、设备的 ioctl、连路径上的 Unix 套接字）能管就管，这台内核没有的就少管那几样；沙盒外建的抽象
//! 套接字（第 6 版起）也能管就管。连套接字要管，是因为 D-Bus、Docker 这类系统服务能替命令在沙盒外读写（2026-09-29
//! 项目主人定：沙盒不管网络，但这类系统服务照样挡）。
//!
//! 规则排成三种（[`Layer`]）：从头建的一层（[`Layer::Whole`]，和施工 V-47 以前一样）；核心缓存的那一层（[`Layer::Base`]，
//! 不管「连 Unix 套接字」）和助手每条命令再叠的一小层（[`Layer::Unix`]，只管「连 Unix 套接字」和「跨目录改名、链接」）。
//! 后两层叠起来和第一种放行的一样，理由见 `docs/construction/V-47-沙盒规则集缓存.md`「两层」。

use std::io;
use std::path::{Path, PathBuf};

use landlock::{
    ABI, Access, AccessFs, BitFlags, CompatLevel, Compatible, PathBeneath, PathFd, PathFdError,
    Ruleset, RulesetAttr, RulesetCreated, RulesetCreatedAttr, RulesetError, Scope,
};

use crate::Spec;

use super::reads::plan;

/// 至少要管得住的那一版。
const REQUIRED: ABI = ABI::V2;

/// 这个库认得的最新一版：能管就管到这一版。
pub(super) const LATEST: ABI = ABI::V9;

/// 规则排成哪一种（见模块说明）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    /// 从头建的一层：全部。
    Whole,
    /// 核心缓存的那一层：除了「连 Unix 套接字」的全部；`sockets` 不在里面。
    Base,
    /// 每条命令叠的一小层：「连 Unix 套接字」和「跨目录改名、链接」，只放 `write`、`/dev/null`、`sockets`。
    Unix,
}

/// 一条规则：给 `path` 放 `access`。`readable` 的是放读的那种（打开了却查不了是什么的跳过，[`build`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rule {
    /// 放在哪。
    pub(crate) path: PathBuf,
    /// 放什么。
    pub(crate) access: BitFlags<AccessFs>,
    /// 放读的那种。
    pub(crate) readable: bool,
}

impl Rule {
    fn new(path: &Path, access: BitFlags<AccessFs>, readable: bool) -> Rule {
        Rule {
            path: path.to_path_buf(),
            access,
            readable,
        }
    }
}

/// 一条规则加的时候那个路径是什么（施工 V-47）：打开它查到的设备号、inode 号、种类（`st_mode` 的种类那几位），或者
/// 查不了时系统的错误号（不在的是 `ENOENT`）。核心缓存时照它认「还是不是那几个文件」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ident {
    /// 查到了。
    Found {
        /// 设备号。
        dev: u64,
        /// inode 号。
        ino: u64,
        /// 种类。
        kind: u32,
    },
    /// 查不了：系统的错误号。
    Error(i32),
}

impl Ident {
    /// 照 `stat` 的结果。
    #[allow(
        clippy::useless_conversion,
        reason = "各架构上 `st_dev`、`st_ino`、`st_mode` 的类型不一样，有的本来就是这个类型"
    )]
    pub(crate) fn of(stat: rustix::io::Result<rustix::fs::Stat>) -> Ident {
        match stat {
            Ok(stat) => Ident::Found {
                dev: u64::from(stat.st_dev),
                ino: u64::from(stat.st_ino),
                kind: u32::from(stat.st_mode) & KIND,
            },
            Err(error) => Ident::Error(error.raw_os_error()),
        }
    }

    /// 写进指纹的字节。
    pub(crate) fn write(self, out: &mut Vec<u8>) {
        match self {
            Ident::Found { dev, ino, kind } => {
                out.push(b'f');
                out.extend(dev.to_le_bytes());
                out.extend(ino.to_le_bytes());
                out.extend(kind.to_le_bytes());
            }
            Ident::Error(code) => {
                out.push(b'e');
                out.extend(code.to_le_bytes());
            }
        }
    }
}

/// `st_mode` 里说种类的那几位（`S_IFMT`）。
const KIND: u32 = 0o170_000;

/// 一条规则加得怎么样（施工 V-47）：加上了（加的时候查到的是什么），不在跳过了，放读的打开了却查不了跳过了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// 加上了。
    Added(Ident),
    /// 不在，跳过了。
    Missing,
    /// 放读的，打开了却查不了是什么，跳过了。
    Undescribable,
}

impl Outcome {
    /// 和指纹里事先查到的 `ident` 对得上：加上了的是同一个文件；不在的事先也不在；查不了的事先也查不了。
    pub(crate) fn matches(self, ident: Ident) -> bool {
        let missing = rustix::io::Errno::NOENT.raw_os_error();
        match (self, ident) {
            (Outcome::Added(found), ident) => found == ident,
            (Outcome::Missing, Ident::Error(code)) => code == missing,
            (Outcome::Undescribable, Ident::Error(code)) => code != missing,
            _ => false,
        }
    }
}

/// 内核的 Landlock 够不够用：建一个要求第 2 版文件操作的规则集，建得成就够。只建不收紧，探测的进程不受影响。
///
/// # Errors
///
/// 不够用：说原话。
pub fn available() -> Result<(), String> {
    ruleset(Layer::Whole).map(drop)
}

/// 内核的 Landlock 管不管得住连 Unix 套接字（施工 MB-1）：连路径上的（第 9 版，`ResolveUnix`）和沙盒外建的抽象套接字（第 6 版）
/// 两样都要管得住。建一个硬要这两样的规则集，建得成就管得住；只建不收紧。
pub fn confines_sockets() -> bool {
    Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::ResolveUnix)
        .and_then(|ruleset| ruleset.scope(Scope::AbstractUnixSocket))
        .and_then(Ruleset::create)
        .is_ok()
}

/// 照规格收紧自己（每条命令从头建）：根目录往下都能列目录、执行；读照 `plan` 一级级放，绕开藏起来的；`write` 整个放行全部，
/// 里面不挖洞（施工 5-14 改，照 dsh：藏不住的会话那边不写进 `hidden`）；`/dev/null` 放行全部。写了 `read` 的（施工 5-12 下）：
/// 只有它列的能读、列目录、执行，根目录不再整个放，`hidden` 不另算。规格里的路径不在的跳过。
///
/// # Errors
///
/// 收不住：说原话（`landlock is not available: …`、`cannot open <路径>: …`、`landlock: …`）。
pub fn confine(spec: &Spec) -> Result<(), String> {
    let created = ruleset(Layer::Whole)?;
    let rules = whole(spec)?;
    let (created, _) = build(created, &rules, None)?;
    restrict(created)
}

/// 从头建的那一层的规则（[`Layer::Whole`]），先后照加的先后。
fn whole(spec: &Spec) -> Result<Vec<Rule>, String> {
    let mut rules = match &spec.read {
        None => base(spec, plan(&spec.hidden)?, Layer::Whole),
        Some(read) => {
            let mut rules: Vec<Rule> = read
                .iter()
                .map(|path| Rule::new(path, AccessFs::from_read(LATEST), true))
                .collect();
            rules.extend(tail(spec, Layer::Whole));
            rules
        }
    };
    // 脚本调工具的通道（施工 M-3）：只放行连这几个套接字。管不了连套接字的旧内核本来就不拦，库照最宽松的兼容级别去掉这一样。
    rules.extend(sockets(spec));
    Ok(rules)
}

/// 没写 `read` 的规格、读照 `plan` 放的规则，先后照加的先后：根目录列目录、执行，`plan` 每条放读，再接 [`tail`]。`layer` 是
/// [`Layer::Whole`] 或 [`Layer::Base`]，`sockets` 不在里面。
pub(crate) fn base(spec: &Spec, plan: Vec<PathBuf>, layer: Layer) -> Vec<Rule> {
    let mut rules = vec![Rule::new(
        Path::new("/"),
        AccessFs::ReadDir | AccessFs::Execute,
        false,
    )];
    rules.extend(
        plan.iter()
            .map(|path| Rule::new(path, AccessFs::from_read(LATEST), true)),
    );
    rules.extend(tail(spec, layer));
    rules
}

/// 读以后的那几条：放开只读的（施工 X-10 中：Landlock 的规则只放行、互相叠加，藏起来的那一片没放行，这里补上一块）每条放读，
/// `write` 每条、`/dev/null` 放全部（[`Layer::Base`] 去掉「连 Unix 套接字」）。
fn tail(spec: &Spec, layer: Layer) -> Vec<Rule> {
    let all = match layer {
        Layer::Base => AccessFs::from_all(LATEST) & !AccessFs::ResolveUnix,
        Layer::Whole | Layer::Unix => AccessFs::from_all(LATEST),
    };
    let mut rules: Vec<Rule> = spec
        .reveal
        .iter()
        .map(|path| Rule::new(path, AccessFs::from_read(LATEST), true))
        .collect();
    rules.extend(spec.write.iter().map(|path| Rule::new(path, all, false)));
    rules.push(Rule::new(Path::new("/dev/null"), all, false));
    rules
}

/// 规格的 `sockets` 每条只放「连 Unix 套接字」。
fn sockets(spec: &Spec) -> impl Iterator<Item = Rule> + '_ {
    spec.sockets
        .iter()
        .map(|path| Rule::new(path, AccessFs::ResolveUnix.into(), false))
}

/// 每条命令叠的那一小层的规则（[`Layer::Unix`]）：`write`、`/dev/null` 放「连 Unix 套接字」和「跨目录改名、链接」，`sockets`
/// 放「连 Unix 套接字」。从头建的那一层里放这两样的就是这几条（读放的 `from_read` 里没有这两样），一条不多、一条不少。
pub(crate) fn unix(spec: &Spec) -> Vec<Rule> {
    let both = AccessFs::ResolveUnix | AccessFs::Refer;
    let mut rules: Vec<Rule> = spec
        .write
        .iter()
        .map(|path| Rule::new(path, both, false))
        .collect();
    rules.push(Rule::new(Path::new("/dev/null"), both, false));
    rules.extend(sockets(spec));
    rules
}

/// 建规则集，照 `layer`：
///
/// - [`Layer::Whole`]：第 2 版的文件操作必须管得住，更新的能管就管，抽象套接字能管就管；
/// - [`Layer::Base`]：同上，只是不管「连 Unix 套接字」；
/// - [`Layer::Unix`]：只管「连 Unix 套接字」和「跨目录改名、链接」，两样都必须管得住（调的一方先问 [`unix_supported`]）。
///
/// 收紧时都设 `no_new_privs`。
///
/// # Errors
///
/// 建不成：`landlock is not available: <原话>`。
pub(crate) fn ruleset(layer: Layer) -> Result<RulesetCreated, String> {
    let unavailable = |error: RulesetError| format!("landlock is not available: {error}");
    let created = match layer {
        Layer::Whole | Layer::Base => {
            let more = match layer {
                Layer::Base => AccessFs::from_all(LATEST) & !AccessFs::ResolveUnix,
                Layer::Whole | Layer::Unix => AccessFs::from_all(LATEST),
            };
            Ruleset::default()
                .set_compatibility(CompatLevel::HardRequirement)
                .handle_access(AccessFs::from_all(REQUIRED))
                .and_then(|ruleset| {
                    ruleset
                        .set_compatibility(CompatLevel::BestEffort)
                        .handle_access(more)
                })
                .and_then(|ruleset| ruleset.scope(Scope::AbstractUnixSocket))
                .and_then(Ruleset::create)
        }
        // 加规则时照最宽松的兼容级别（和另外两种一样）：`/dev/null`、是文件的 `write` 去掉只有目录才有的「跨目录改名、链接」。
        Layer::Unix => Ruleset::default()
            .set_compatibility(CompatLevel::HardRequirement)
            .handle_access(AccessFs::ResolveUnix | AccessFs::Refer)
            .map(|ruleset| ruleset.set_compatibility(CompatLevel::BestEffort))
            .and_then(Ruleset::create),
    };
    Ok(created.map_err(unavailable)?.no_new_privs(true))
}

/// 这台内核的 Landlock 管不管「连 Unix 套接字」（第 9 版起）：只问库认的版本，不建规则集。管不了的，从头建的那一层本来就不管它
/// （库照最宽松的兼容级别去掉），叠的那一小层也就不要。
pub(crate) fn unix_supported() -> bool {
    Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::ResolveUnix)
        .is_ok()
}

/// 把 `rules` 一条条加进 `created`，交回加好的和每条加得怎么样（[`Outcome`]）。`check` 给了的（核心缓存时，施工 V-47），每条
/// 加得怎么样要和它事先查到的对得上（[`Outcome::matches`]），对不上的说路径在建的时候变了。
///
/// 每条：打开（`O_PATH`，跟链接），不在的跳过；放读的打开了却查不了是什么的（断开的 FUSE 挂载，例如守护进程没了的
/// `~/.gvfs`，`fstat` 报「Transport endpoint is not connected」）跳过：它本来就读不了，跳过等于在沙盒里也读不了，不放宽
/// 什么；不跳过的话，[`plan`] 一级级列到它，整个沙盒就起不来、什么命令都跑不了（施工 5-4 补，2026-10-01 项目主人的机器上
/// 撞见）。写的规则不这样：写要放行的路径坏了，照旧报错。是文件的，只放行对文件有意义的那几样（库照最宽松的兼容级别自己
/// 去掉目录才有的）。
///
/// # Errors
///
/// 打不开（不在的除外）：`cannot open <路径>: <原话>`；加不上：`landlock: <原话>`；和 `check` 对不上：`changed: <路径>`。
pub(crate) fn build(
    mut created: RulesetCreated,
    rules: &[Rule],
    check: Option<&[Ident]>,
) -> Result<(RulesetCreated, Vec<Outcome>), String> {
    let mut outcomes = Vec::with_capacity(rules.len());
    for (at, rule) in rules.iter().enumerate() {
        let opened = match PathFd::new(&rule.path) {
            Ok(fd) => Ok(fd),
            Err(PathFdError::OpenCall { source, .. })
                if source.kind() == io::ErrorKind::NotFound =>
            {
                Err(Outcome::Missing)
            }
            Err(PathFdError::OpenCall { source, .. }) => {
                return Err(format!("cannot open {}: {source}", rule.path.display()));
            }
            Err(error) => return Err(format!("cannot open {}: {error}", rule.path.display())),
        };
        let outcome = match opened {
            Err(outcome) => outcome,
            Ok(fd) => {
                let ident = Ident::of(rustix::fs::fstat(&fd));
                if rule.readable && matches!(ident, Ident::Error(_)) {
                    Outcome::Undescribable
                } else {
                    created = created
                        .add_rule(PathBeneath::new(fd, rule.access))
                        .map_err(|error| format!("landlock: {error}"))?;
                    Outcome::Added(ident)
                }
            }
        };
        if let Some(check) = check
            && !check.get(at).is_some_and(|ident| outcome.matches(*ident))
        {
            return Err(format!("changed: {}", rule.path.display()));
        }
        outcomes.push(outcome);
    }
    Ok((created, outcomes))
}

/// 收紧自己。
///
/// # Errors
///
/// 收不住：`landlock: <原话>`；内核说没生效：`landlock is not available: not enforced`。
pub(crate) fn restrict(created: RulesetCreated) -> Result<(), String> {
    let status = created
        .restrict_self()
        .map_err(|error| format!("landlock: {error}"))?;
    match status.ruleset {
        landlock::RulesetStatus::NotEnforced => {
            Err("landlock is not available: not enforced".to_string())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
