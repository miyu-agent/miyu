//! 扩展的工具交的效果（施工 XG-1 上，设计 `63-扩展工具的效果和声明.md` 第三、四节）：核心不信扩展说的内容，只信它说「哪条
//! 改了」。
//!
//! - 只收四种：`file.changed`、`file.read`、`file.trashed`、`todo.written`；别的种类整条丢，不可信的东西不进日志；
//! - 文件的路径换成真实位置后要是这次调用过了链的（[`Judged`]）：改、删的要是写的那几条，读的读写都行。没声明 `paths` 的工具
//!   一条都不收；扩展报 `~/.ssh/…` 这类没判过的，核心不去读，进不了她的上下文、压后重读；
//! - 改前是发 `tool.call` 之前核心自己读的那一份（[`befores`]），改后、读的哈希是回应到了核心现读的：报假内容骗不了看差异的人；
//!   带了哈希、原文的要和读的对得上；
//! - 丢的记一行 `WARN tool effect dropped`，`why` 是 [`Why`] 之一；结果别的格照常，不算出错，不告诉她。
//!
//! 碰磁盘，在阻塞线程里调。读成的照内置工具的样子做成 [`miyu_tool::Effect`]，交给执行器存 blob、记她看过的。

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use miyu_fs::{OpenError, open_file, resolve};
use miyu_kernel::event::TodoWritten;
use miyu_kernel::id::ContentHash;
use miyu_tool::{Call, Done, Effect};

use super::declared::{self, PathParameter};

/// 一份至多读多少字节（设计 63 第三节第 3 条）：再大的不读，它的 `file.changed`、`file.read` 丢掉（`too_big`）。
const MOST_BYTES: u64 = 20 * 1024 * 1024;

/// 收的四种效果（`kind`）：写成常量，和协议方法名一个样子的字面量不放在 `match` 的分支上（门禁照那样认分发的方法）。
const CHANGED: &str = "file.changed";
/// 读了。
const READ: &str = "file.read";
/// 移进了回收站。
const TRASHED: &str = "file.trashed";
/// 换上一整份待办。
const TODOS: &str = "todo.written";

/// 回收站里的位置至多几个字（第四节第 1 条）：只给人看。
const MOST_TRASH_CHARS: usize = 4096;

/// 链判过的一条：真实位置，是不是写。照工具报的先后。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Judged {
    pub(super) real: PathBuf,
    pub(super) write: bool,
}

/// 一个文件某一刻的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Content {
    /// 不在：改前不在的是新建。
    Missing,
    /// 整份的字节。
    Bytes(Vec<u8>),
    /// 超过 [`MOST_BYTES`]，没读。
    TooBig,
    /// 不是普通文件、读不了。
    Unreadable,
}

/// 发 `tool.call` 之前要写的那几条各自的改前（第三节第 3 条）：不写的不读，不多花一次磁盘。
pub(super) type Befores = BTreeMap<PathBuf, Content>;

/// 这次调用链判过的：照声明 `paths` 从参数报的每一条配上 [`Call::checked`] 里判的时候换成的真实位置（两边照报的先后）。
/// 没带判过的（测试里直接造的调用）、没声明 `paths` 的是空的：文件效果一条都不收。
pub(super) fn judged(paths: &[PathParameter], call: &Call) -> Vec<Judged> {
    let Some(checked) = &call.checked else {
        return Vec::new();
    };
    declared::targets(paths, &call.args)
        .into_iter()
        .zip(checked.iter())
        .map(|(target, real)| Judged {
            real: real.clone(),
            write: target.write,
        })
        .collect()
}

/// 要写的那几条的改前（第三节第 3 条）：发 `tool.call` 之前在阻塞线程里读；一条要写的都没有的不碰磁盘。
pub(super) async fn befores(judged: &[Judged]) -> Befores {
    if !judged.iter().any(|one| one.write) {
        return Befores::new();
    }
    let judged = judged.to_vec();
    tokio::task::spawn_blocking(move || read_befores(&judged))
        .await
        .unwrap_or_default()
}

/// 读一份改前：`judged` 里要写的那几条。
fn read_befores(judged: &[Judged]) -> Befores {
    judged
        .iter()
        .filter(|one| one.write)
        .map(|one| (one.real.clone(), copy(&one.real)))
        .collect()
}

/// 查工具 `tool` 回应里还没查的效果 `raw`，收下的接在 `done` 已有的效果前面：在阻塞线程里读盘。`call` 是这次调用，`judged`、
/// `befores` 是发之前算好、读好的。
pub(super) async fn vetted(
    tool: &str,
    mut done: Done,
    raw: Option<Value>,
    (call, judged, befores): (&Call, Vec<Judged>, Befores),
) -> Done {
    if raw.is_none() {
        return done;
    }
    let tool = tool.to_string();
    let (cwd, home, error) = (call.cwd.clone(), call.home.clone(), done.error);
    let vetted = tokio::task::spawn_blocking(move || {
        let context = Context {
            tool: &tool,
            cwd: &cwd,
            home: home.as_deref(),
            judged: &judged,
            befores: &befores,
            error,
        };
        vet(raw, &context)
    })
    .await
    .unwrap_or_default();
    done.effects.splice(0..0, vetted);
    done
}

/// 读 `real` 这一刻的内容：照「安全地打开」开（不跟最后一层链接、不是普通文件的不读），至多 [`MOST_BYTES`]。
fn copy(real: &Path) -> Content {
    let file = match open_file(real) {
        Ok(file) => file,
        Err(OpenError::NotFound) => return Content::Missing,
        Err(_) => return Content::Unreadable,
    };
    let mut bytes = Vec::new();
    match file.take(MOST_BYTES + 1).read_to_end(&mut bytes) {
        Ok(_) if bytes.len() as u64 > MOST_BYTES => Content::TooBig,
        Ok(_) => Content::Bytes(bytes),
        Err(_) => Content::Unreadable,
    }
}

/// 一条为什么丢（`WARN tool effect dropped` 的 `why`，第四节第 3 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Why {
    /// 路径不是这次调用过了链的，或者要写却只判了读。
    NotChecked,
    /// 说改了，改前改后一样。
    Unchanged,
    /// 带的哈希、原文和核心读的对不上；说改了、读了却不是一个普通文件；说删了却还在。
    Mismatch,
    /// 文件超过 20 MiB，核心不读。
    TooBig,
    /// 不收的种类。
    Kind,
    /// 写法不对；同一种同一个文件报了两次的，后面的也算。
    Shape,
}

impl Why {
    /// 运行日志里的写法。
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Why::NotChecked => "not_checked",
            Why::Unchanged => "unchanged",
            Why::Mismatch => "mismatch",
            Why::TooBig => "too_big",
            Why::Kind => "kind",
            Why::Shape => "shape",
        }
    }
}

/// 查效果要知道的：哪件工具、照哪个工作目录和家目录换路径、链判过的、改前、回应是不是出错。
pub(super) struct Context<'a> {
    pub(super) tool: &'a str,
    pub(super) cwd: &'a str,
    pub(super) home: Option<&'a Path>,
    pub(super) judged: &'a [Judged],
    pub(super) befores: &'a Befores,
    /// 回应说出了错：文件的照收（改了一半，改了的是真的），待办不收（同 `state`）。
    pub(super) error: bool,
}

/// 查回应里的 `effects`（不写、写 `null` 的是没有）：收下的照先后交回，丢的各记一行。
pub(super) fn vet(raw: Option<Value>, context: &Context) -> Vec<Effect> {
    let items = match raw {
        None | Some(Value::Null) => return Vec::new(),
        Some(Value::Array(items)) => items,
        Some(_) => {
            dropped(context.tool, "-", Why::Shape);
            return Vec::new();
        }
    };
    let mut afters = BTreeMap::new();
    let mut reported = BTreeSet::new();
    let mut kept = Vec::new();
    for item in items {
        let kind = item["kind"].as_str().unwrap_or("-").to_string();
        if kind == TODOS && context.error {
            continue;
        }
        match one(item, context, &mut afters) {
            Ok(effect) => {
                // 同一种同一个文件只收第一条：重复的什么都不添，还会让核心多读几遍。
                let key = (kind.clone(), path_of(&effect));
                if key.1.is_some() && !reported.insert(key) {
                    dropped(context.tool, &kind, Why::Shape);
                } else {
                    kept.push(effect);
                }
            }
            Err(why) => dropped(context.tool, &kind, why),
        }
    }
    kept
}

/// 记一行丢了的。
fn dropped(tool: &str, kind: &str, why: Why) {
    tracing::warn!(target: "miyu::endpoint", tool, kind, why = why.as_str(), "tool effect dropped");
}

/// 文件效果的路径；待办没有。
fn path_of(effect: &Effect) -> Option<PathBuf> {
    match effect {
        Effect::Read { path, .. } | Effect::Changed { path, .. } | Effect::Trashed { path, .. } => {
            Some(path.clone())
        }
        _ => None,
    }
}

/// `file.changed`：带了的哈希、原文要和核心读的对得上。
#[derive(Deserialize)]
struct Changed {
    path: String,
    #[serde(default)]
    before: Option<ContentHash>,
    #[serde(default)]
    after: Option<ContentHash>,
    #[serde(default)]
    before_text: Option<String>,
    #[serde(default)]
    after_text: Option<String>,
}

/// `file.read`：哈希由核心读盘算，扩展写了也不看。
#[derive(Deserialize)]
struct FileRead {
    path: String,
    #[serde(default)]
    lines: Option<[u64; 2]>,
}

/// `file.trashed`：回收站里的位置只给人看。
#[derive(Deserialize)]
struct Trashed {
    path: String,
    trash: String,
}

/// 查一条。`afters` 是这次回应里已经读过的改后，同一个文件不读第二遍。
fn one(
    item: Value,
    context: &Context,
    afters: &mut BTreeMap<PathBuf, Content>,
) -> Result<Effect, Why> {
    let kind = item["kind"].as_str().ok_or(Why::Shape)?.to_string();
    match kind.as_str() {
        CHANGED => {
            let changed = serde_json::from_value::<Changed>(item).map_err(|_| Why::Shape)?;
            let real = checked_path(context, &changed.path, true)?;
            let before = match context.befores.get(&real) {
                Some(Content::Bytes(bytes)) => Some(bytes.clone()),
                Some(Content::Missing) => None,
                Some(Content::TooBig) => return Err(Why::TooBig),
                Some(Content::Unreadable) | None => return Err(Why::Mismatch),
            };
            let after = bytes(now(afters, &real))?;
            if before.as_ref() == Some(&after) {
                return Err(Why::Unchanged);
            }
            agrees(&changed, before.as_deref(), &after)?;
            Ok(Effect::Changed {
                path: real,
                before,
                after,
            })
        }
        READ => {
            let read = serde_json::from_value::<FileRead>(item).map_err(|_| Why::Shape)?;
            if read.lines.is_some_and(|[from, to]| from == 0 || from > to) {
                return Err(Why::Shape);
            }
            let real = checked_path(context, &read.path, false)?;
            let content = bytes(now(afters, &real))?;
            Ok(Effect::Read {
                path: real,
                lines: read.lines,
                hash: ContentHash::of(&content),
            })
        }
        TRASHED => {
            let trashed = serde_json::from_value::<Trashed>(item).map_err(|_| Why::Shape)?;
            if trashed.trash.chars().count() > MOST_TRASH_CHARS {
                return Err(Why::Shape);
            }
            let real = checked_path(context, &trashed.path, true)?;
            // 说删了的要真不在了：链接本身也不算在。
            if std::fs::symlink_metadata(&real).is_ok() {
                return Err(Why::Mismatch);
            }
            Ok(Effect::Trashed {
                path: real,
                trash: trashed.trash,
            })
        }
        TODOS => todos(item),
        _ => Err(Why::Kind),
    }
}

/// `todo.written`：只查写法（同内置的 `todowrite`）。`done` 写 `null` 的当没写（施工 XG-1 补：同 `effects`、`state` 的
/// `null`），写成别的样子的整条丢。
fn todos(mut item: Value) -> Result<Effect, Why> {
    if item["done"].is_null()
        && let Some(fields) = item.as_object_mut()
    {
        fields.remove("done");
    }
    serde_json::from_value::<TodoWritten>(item)
        .map(Effect::TodoWritten)
        .map_err(|_| Why::Shape)
}

/// 把扩展报的 `path` 换成真实位置（同 [`miyu_tool::Call::real`]），要是链判过的一条；`write` 的要是判了写的。
fn checked_path(context: &Context, path: &str, write: bool) -> Result<PathBuf, Why> {
    let real = resolve(Path::new(context.cwd), context.home, path).map_err(|_| Why::NotChecked)?;
    context
        .judged
        .iter()
        .find(|one| one.real == real && (one.write || !write))
        .map(|one| one.real.clone())
        .ok_or(Why::NotChecked)
}

/// `real` 现在的内容：这次回应里读过的照那一份。
fn now<'a>(afters: &'a mut BTreeMap<PathBuf, Content>, real: &Path) -> &'a Content {
    afters
        .entry(real.to_path_buf())
        .or_insert_with(|| copy(real))
}

/// 读成了的字节；太大的、不在的、读不了的照丢的原因交回。
fn bytes(copy: &Content) -> Result<Vec<u8>, Why> {
    match copy {
        Content::Bytes(bytes) => Ok(bytes.clone()),
        Content::TooBig => Err(Why::TooBig),
        Content::Missing | Content::Unreadable => Err(Why::Mismatch),
    }
}

/// 扩展带了的哈希、原文要和核心读的一样；没带的不查。改前不在（新建）却带了改前的，对不上。
fn agrees(changed: &Changed, before: Option<&[u8]>, after: &[u8]) -> Result<(), Why> {
    let hash_fits = |claimed: &Option<ContentHash>, content: Option<&[u8]>| match claimed {
        None => true,
        Some(hash) => content.is_some_and(|content| ContentHash::of(content) == *hash),
    };
    let text_fits = |claimed: &Option<String>, content: Option<&[u8]>| match claimed {
        None => true,
        Some(text) => content == Some(text.as_bytes()),
    };
    let fits = hash_fits(&changed.before, before)
        && hash_fits(&changed.after, Some(after))
        && text_fits(&changed.before_text, before)
        && text_fits(&changed.after_text, Some(after));
    if fits { Ok(()) } else { Err(Why::Mismatch) }
}
