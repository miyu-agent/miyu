//! 找脚本（施工 X-12 上，`docs/blueprint/skills.md`「脚本」，设计 `31-软件包.md` 第五节）：一个脚本是一个文件，开头的注释写
//! 说明、参数（`miyu_policy::script`）。从两处找：
//!
//! - 装了的软件包（清单读成了的）的 `scripts/<文件>`；
//! - 本机自己放的 `home/<账号>/scripts/<文件>`。
//!
//! 同一个工具名的自己放的赢；两个包撞了，出厂的先于家目录、同一层照包的编号留前一个（装的时候撞了的装不上，施工 F-8 六补），
//! 后来的跳过、记下被谁盖住（施工 X-13 中）。读不了的、不是字的、
//! 开头写错的跳过，交回给调的一方记。子文件夹、
//! 点开头的不算。脚本可以是链接（施工 X-37，`links.rs`）：照它指到的认，[`Script::file`] 记链接本身（放行出沙盒时
//! 照它看是不是链接）。碰磁盘，调的一方放在阻塞线程里。

use std::path::{Path, PathBuf};

use miyu_kernel::id::{AccountId, VenueId};
use miyu_policy::script::{self, ScriptHead};

use crate::links::{self, Fence, Kind};
use crate::packages::Found;
use crate::root::DataRoot;
use crate::skills::Skipped;

/// 家目录里自己放的脚本登记工具时挂的包编号（施工 X-12 中）：包编号的写法里没有 `~`，撞不上真的包。
pub const HOME_PACKAGE: &str = "~home";

/// 一个找到的脚本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// 开头读出来的。
    pub head: ScriptHead,
    /// 脚本文件：是链接的就是链接本身（施工 X-37：执行器照它看是不是链接，起的时候照它指到的）。
    pub file: PathBuf,
    /// 哪个软件包带的；自己放的没有。
    pub package: Option<String>,
}

/// 账号 `account` 自己放脚本的文件夹：`home/<账号>/scripts/`，还没建的也照写（施工 X-27 补，网页「打开目录」要）。
pub fn own_dir(root: &DataRoot, account: &AccountId) -> PathBuf {
    root.account_dir(account).join("scripts")
}

/// 脚本的状态目录都在数据根 `data_root` 的这下面（施工 X-44）：`state/scripts/`。卸包、`miyu check` 照它找。
pub fn state_root(data_root: &Path) -> PathBuf {
    data_root.join("state").join("scripts")
}

/// 场所会话的那一层目录名的开头（施工 X-44 补）：包的编号、[`HOME_PACKAGE`] 都不这样开头，分得清这一层是场所。
const VENUE_MARK: char = '@';

/// 场所写成的目录名最多几个字：再长的给不了状态目录（场所编号最长 128 字节，全要转写才会这么长）。
const VENUE_DIR_MOST: usize = 200;

/// 本机的会话的场所：状态目录不分场所。
const LOCAL: &str = "local";

/// 场所 `venue` 落到目录名（施工 X-44 补）：`@` 后面照字节写，小写字母、数字、`-` 原样，别的写成 `_` 加两位小写十六进制。
/// 可逆、不会撞，大小写不分的盘上也撞不上，碰不到 Windows 的保留名、末尾的点和空格。写出来超过 200 个字的没有。
pub fn venue_dir(venue: &VenueId) -> Option<String> {
    let mut dir = String::from(VENUE_MARK);
    for byte in venue.as_str().bytes() {
        if kept(byte) {
            dir.push(char::from(byte));
        } else {
            dir.push_str(&format!("_{byte:02x}"));
        }
    }
    (dir.len() <= VENUE_DIR_MOST).then_some(dir)
}

/// 反过来：照 [`venue_dir`] 的写法读回场所。不是 `@` 开头的、写法不对的、不是唯一那一种写法的没有。
pub fn venue_of(dir: &str) -> Option<VenueId> {
    let mut bytes = Vec::new();
    let mut rest = dir.strip_prefix(VENUE_MARK)?.bytes();
    while let Some(byte) = rest.next() {
        let byte = match byte {
            b'_' => {
                let written = hex(rest.next()?)? << 4 | hex(rest.next()?)?;
                (!kept(written)).then_some(written)?
            }
            byte => kept(byte).then_some(byte)?,
        };
        bytes.push(byte);
    }
    VenueId::parse(&String::from_utf8(bytes).ok()?).ok()
}

/// 场所写进目录名时原样留下的字节：小写字母、数字、`-`。
fn kept(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
}

/// 一位小写十六进制的值；大写的不认（只认 [`venue_dir`] 写出来的那一种）。
fn hex(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

/// 账号 `account` 在场所 `venue` 跑来处是 `source`（包的编号，家目录里放的是 [`HOME_PACKAGE`]）、工具名是 `tool` 的脚本时给它的
/// 状态目录（施工 X-44，`MIYU_SCRIPT_STATE_DIR`）：本机的会话、没说场所的是 `state/scripts/<账号>/<来处>/<工具名>/`；场所会话
/// 多一层场所（施工 X-44 补）：`state/scripts/<账号>/@<场所>/<来处>/<工具名>/`。场所写不成目录名的没有。不建：跑之前脚本工具建。
pub fn state_dir(
    data_root: &Path,
    account: &AccountId,
    venue: Option<&VenueId>,
    source: &str,
    tool: &str,
) -> Option<PathBuf> {
    let mut dir = state_root(data_root).join(account.as_str());
    if let Some(venue) = venue.filter(|venue| venue.as_str() != LOCAL) {
        dir.push(venue_dir(venue)?);
    }
    Some(dir.join(source).join(tool))
}

/// 账号 `account` 在场所 `venue` 跑来处是 `source`、工具名是 `tool` 的脚本时给它的缓存目录（施工 Y-11 上，
/// `MIYU_SCRIPT_CACHE_DIR`）：`home/<账号>/cache/<来处>/<工具名>/`，场所会话多一层场所（同 [`state_dir`]）。场所写不成目录名的
/// 没有。不建：跑之前脚本工具建。
pub fn cache_dir(
    data_root: &Path,
    account: &AccountId,
    venue: Option<&VenueId>,
    source: &str,
    tool: &str,
) -> Option<PathBuf> {
    // 同 `DataRoot::cache`：调的一方手里只有数据根的路径。
    let mut dir = data_root
        .join("home")
        .join(account.as_str())
        .join(crate::root::CACHE);
    if let Some(venue) = venue.filter(|venue| venue.as_str() != LOCAL) {
        dir.push(venue_dir(venue)?);
    }
    Some(dir.join(source).join(tool))
}

/// 一格状态目录（施工 X-44）：`state/scripts/<账号>/<来处>/<工具名>/`，场所会话的多一层场所（施工 X-44 补）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// 来处：包的编号，或者 [`HOME_PACKAGE`]。
    pub source: String,
    /// 工具名。
    pub tool: String,
    /// 目录。
    pub path: PathBuf,
}

/// 数据根 `data_root` 里所有账号的状态目录，照路径排（施工 X-44：`miyu check` 照它报没有脚本的那几格）。读不了的那一层跳过。
pub fn states(data_root: &Path) -> Vec<State> {
    let mut found = Vec::new();
    for from in sources(data_root) {
        for path in folders(&from) {
            if let (Some(source), Some(tool)) = (name(&from), name(&path)) {
                found.push(State { source, tool, path });
            }
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// 每个账号下放来处的那几层：账号那一层，加它下面每个场所那一层（施工 X-44 补）。
fn places(data_root: &Path) -> Vec<PathBuf> {
    let mut places = Vec::new();
    for account in folders(&state_root(data_root)) {
        let venues = folders(&account)
            .into_iter()
            .filter(|dir| name(dir).as_deref().and_then(venue_of).is_some());
        places.extend(venues);
        places.push(account);
    }
    places
}

/// 每个来处的文件夹：[`places`] 下面不是场所那一层的。
fn sources(data_root: &Path) -> Vec<PathBuf> {
    places(data_root)
        .iter()
        .flat_map(|place| folders(place))
        .filter(|dir| !name(dir).is_some_and(|name| name.starts_with(VENUE_MARK)))
        .collect()
}

/// 路径最后一段的名字。
fn name(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// 目录 `dir` 下一层的文件夹；读不了的没有。
fn folders(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

/// 卸掉来处是 `source` 的包（施工 X-44）：删掉每个账号下的 `state/scripts/<账号>/<来处>/`，场所那一层下面的
/// `<账号>/@<场所>/<来处>/` 也删（施工 X-44 补）。本来就没有的不算错。
///
/// # Errors
///
/// 删不掉的那一个。
pub fn forget_states(data_root: &Path, source: &str) -> std::io::Result<()> {
    for place in places(data_root) {
        match std::fs::remove_dir_all(place.join(source)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

/// 找脚本：装了的软件包 `packages` 里的，加数据根 `root` 里账号 `account` 自己放的。照工具名排。
pub fn find(
    packages: &[Found],
    root: &DataRoot,
    account: &AccountId,
) -> (Vec<Script>, Vec<Skipped>) {
    let mut scripts: Vec<Script> = Vec::new();
    let mut skipped = Vec::new();
    let own = own_dir(root, account);
    let mut places: Vec<(PathBuf, Option<String>)> = vec![(own, None)];
    let mut installed: Vec<&Found> = packages.iter().filter(|found| found.read.is_ok()).collect();
    // 出厂的先于家目录，同一层照编号（施工 F-8 六补）：核心升级后出厂的新带了同名的，出厂的赢（设计 31 定了的 J）。
    installed.sort_by(|a, b| (a.layer, &a.id).cmp(&(b.layer, &b.id)));
    places.extend(
        installed
            .into_iter()
            .map(|found| (found.files_dir().join("scripts"), Some(found.id.clone()))),
    );
    let fence = Fence::of(root, account);
    for (place, package) in places {
        let (entries, unlisted) = links::list(&place, Kind::File, &fence, |_| true);
        skipped.extend(unlisted);
        for entry in entries {
            let file = entry.path;
            match head(&file, &entry.name) {
                Ok(head) => match scripts.iter().find(|found| found.head.name == head.name) {
                    Some(winner) => skipped.push(Skipped {
                        by: Some(winner.file.clone()),
                        ..Skipped::new(
                            file,
                            format!(
                                "script {:?} is already provided by {}",
                                head.name,
                                winner.file.display()
                            ),
                        )
                    }),
                    None => scripts.push(Script {
                        head,
                        file,
                        package: package.clone(),
                    }),
                },
                Err(why) => skipped.push(Skipped::new(file, why)),
            }
        }
    }
    scripts.sort_by(|a, b| a.head.name.cmp(&b.head.name));
    (scripts, skipped)
}

/// 包文件夹 `dir` 带的脚本（施工 X-13 上，装之前给人看）：`scripts/` 下开头读得成的工具名，照文件名排；读不成的另交回为什么
/// （施工 X-13 补）。
pub fn carried(dir: &Path) -> (Vec<String>, Vec<Skipped>) {
    let mut names = Vec::new();
    let mut skipped = Vec::new();
    let (entries, unlisted) =
        links::list(&dir.join("scripts"), Kind::File, &Fence::anywhere(), |_| {
            true
        });
    skipped.extend(unlisted);
    for entry in entries {
        match head(&entry.path, &entry.name) {
            Ok(head) => names.push(head.name),
            Err(why) => skipped.push(Skipped::new(entry.path, why)),
        }
    }
    (names, skipped)
}

/// 读文件名是 `name` 的脚本 `file` 的开头：读不了、不是字、写错了的交回为什么。
fn head(file: &Path, name: &str) -> Result<ScriptHead, String> {
    let bytes = std::fs::read(file).map_err(|error| format!("not readable: {error}"))?;
    let text = String::from_utf8(bytes).map_err(|_| "not text".to_string())?;
    script::read(name, &text).map_err(|problem| problem.to_string())
}

#[cfg(test)]
mod tests;
