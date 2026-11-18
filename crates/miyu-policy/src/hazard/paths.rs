//! 删的目标照字面换成绝对路径再判（施工 5-17；施工 AS-9 从 `rules.rs` 挪出来，加上通配和 `cd`）：`~`、`$HOME` 接家目录，
//! 相对的接这时的当前目录（同一条命令里 `cd` 过的照 `cd` 到的，说不清的交回空的），`.`、`..` 照字面去掉。通配的照它能删到
//! 的最大的那一个算：`dir/*`、`dir/?*`、`dir/{*,.*}` 照删 `dir`，`~/.miyu*` 照删数据根。

use super::Kind;
use super::Places;
use super::lex::Word;

/// 通配的字。
const GLOB: [char; 4] = ['*', '?', '[', '{'];

/// 删一个目标：照字面换成绝对路径（通配的照它删得到的最大的那一个），再照 [`Places`] 判。`cwd` 是这时的当前目录，`cd` 到
/// 说不清的地方以后是空的（相对的目标照说不清算）。
pub(super) fn target(
    word: &Word,
    places: &Places<'_>,
    cwd: Option<&str>,
    recursive: bool,
) -> Option<Kind> {
    let Some(path) = absolute(word, places, cwd) else {
        return recursive.then_some(Kind::DeleteUnresolved);
    };
    judge(&path, places, false)
}

/// 挪走、扔进回收站一个来源（施工 RO-4补，2026-11-18 项目主人定「只管数据根」）：照字面换成绝对路径（通配的照它挪得到的最大的
/// 那一个），是数据根自己或装着它的报删数据根，别的（家目录、根、系统目录、工作目录也在内）不报，同 RO-4 以前。说不清的不报。
pub(super) fn moved_away(word: &Word, places: &Places<'_>, cwd: Option<&str>) -> Option<Kind> {
    let path = absolute(word, places, cwd)?;
    holds_data_root(&path, places).then_some(Kind::DeleteDataRoot)
}

/// 规整过的绝对路径 `path`（通配的照它的最大的那一个）是数据根自己或装着它的。
pub(super) fn holds_data_root(path: &str, places: &Places<'_>) -> bool {
    inside(&widest(path, places), places.data_root)
}

/// `of` 是 `dir` 自己或在它里面。
fn inside(dir: &str, of: &str) -> bool {
    of == dir || of.starts_with(&format!("{}/", dir.trim_end_matches('/')))
}

/// 照 [`Places`] 判一个规整过的绝对路径 `path`（`/` 分隔，不以 `/` 结尾）：通配的先照它删得到的最大的那一个算。`drives` 是
/// Windows 的写法（施工 WN-1，`windows/paths.rs`）：第一段是盘符（`/c:/users/me`），盘符本身算根目录，盘符下一级算系统目录。
pub(super) fn judge(path: &str, places: &Places<'_>, drives: bool) -> Option<Kind> {
    let path = widest(path, places);
    let depth = path.split('/').filter(|part| !part.is_empty()).count();
    let root = usize::from(drives);
    if depth <= root {
        return Some(Kind::DeleteRoot);
    }
    if let Some(home) = places.home
        && inside(&path, home)
    {
        return Some(Kind::DeleteHome);
    }
    // 只有数据根自己和装着它的（施工 RO-4，2026-11-18 项目主人定「开放下数据根不特殊」）：里面的单个文件、目录同别处，
    // 和 `trash` 那件工具一个口径。原来连 `rm ~/.miyu/x.log` 也问。
    if inside(&path, places.data_root) {
        return Some(Kind::DeleteDataRoot);
    }
    if depth == root + 1 {
        return Some(Kind::DeleteSystem);
    }
    if inside(&path, places.cwd) {
        return Some(Kind::DeleteWorkingDir);
    }
    None
}

/// 通配的路径删得到的最大的那一个（施工 AS-9）：通配只在最后一段才看。整段都是通配（`*`、`.*`、`?*`、`{*,.*}`）的照删它
/// 所在的目录；`前缀*`（后面只有 `*`）的照删同一层里要保护的那几个（家目录、数据根、工作目录，或者通往它们的那一级）里名字
/// 合前缀的。别的（`*.o`、通配在中间那一段）照原样：根下面的照原样也是根下一级（系统目录），别处的认不出什么。
fn widest(path: &str, places: &Places<'_>) -> String {
    let Some(glob) = path.find(GLOB) else {
        return path.to_string();
    };
    let slash = path[..glob].rfind('/').unwrap_or(0);
    let (dir, last) = (&path[..slash], &path[slash + 1..]);
    if last.contains('/') {
        return path.to_string();
    }
    let dir = if dir.is_empty() { "/" } else { dir };
    if last.contains('*')
        && last
            .chars()
            .all(|c| matches!(c, '*' | '?' | '.' | '{' | '}' | ','))
    {
        return dir.to_string();
    }
    let at = last.find(GLOB).unwrap_or(last.len());
    let (prefix, rest) = last.split_at(at);
    // 前缀后面只有 `*` 的才是「名字以它开头的都算」；`*.log` 这类有尾巴的删不到要保护的那几个。
    if prefix.is_empty() || !rest.chars().all(|c| c == '*') {
        return path.to_string();
    }
    let below = |place: &str| -> Option<String> {
        let rest = place
            .strip_prefix(dir.trim_end_matches('/'))?
            .strip_prefix('/')?;
        let name = rest.split('/').next()?;
        name.starts_with(prefix)
            .then(|| format!("{}/{name}", dir.trim_end_matches('/')))
    };
    let guarded = [places.home, Some(places.data_root), Some(places.cwd)];
    // 根下面的不用另算：照原样就是根下一级，照系统目录判。
    guarded
        .into_iter()
        .flatten()
        .find_map(below)
        .unwrap_or_else(|| path.to_string())
}

/// 照字面换成绝对路径，去掉 `.`、`..`、多余的 `/`：`~`、`$HOME`、`${HOME}` 开头的接家目录，相对的接 `cwd`（空的交回空的）。
/// 别的变量、命令替换说不清，交回空的。
pub(super) fn absolute(word: &Word, places: &Places<'_>, cwd: Option<&str>) -> Option<String> {
    let text = word.text.as_str();
    let (base, rest) = if let Some(rest) = text
        .strip_prefix("${HOME}")
        .or_else(|| text.strip_prefix("$HOME"))
    {
        (places.home?, rest)
    } else if word.unresolved {
        return None;
    } else if text == "~" || text.starts_with("~/") {
        (places.home?, &text[1..])
    } else if text.starts_with('/') {
        ("/", text)
    } else {
        (cwd?, text)
    };
    if rest.contains('$') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/').chain(rest.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    Some(format!("/{}", parts.join("/")))
}
