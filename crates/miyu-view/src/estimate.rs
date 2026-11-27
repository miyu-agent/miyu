//! 编辑、写入加减的行数，照参数估（结果来之前，`internal/blueprint/view.md`「种类」`tool` 的 `diff`）：写入的全部算加上的，
//! 编辑的每一处按行比。和终端原来照参数算的一样。

use std::borrow::Cow;

use serde::Deserialize;
use serde_json::Value;
use similar::{ChangeTag, TextDiff};

use miyu_kernel::tool::shape;

use crate::entry::Diff;

/// 写入的参数里要的那一格。
#[derive(Deserialize)]
struct Write {
    content: String,
}

/// 编辑的参数里要的那一格。
#[derive(Deserialize)]
struct Edit {
    edits: Vec<Piece>,
}

/// 编辑的一处。
#[derive(Deserialize)]
struct Piece {
    old_string: String,
    new_string: String,
}

/// 编辑、写入的参数规整成标准写法的原文（施工 ED-3，[`shape::standard`]）：顶层直接写一处、`edits` 写成一个对象、
/// `path`、`oldText` 这些别名的，标题、估的行数照它；本来就是标准写法的、读不懂的没有。条目的 `args` 照旧是原文。
pub(crate) fn standard(args: &str) -> Option<String> {
    match shape::standard(args) {
        Cow::Owned(standard) => Some(standard),
        Cow::Borrowed(_) => None,
    }
}

/// 参数原文解开：读不懂的是 `null`。
pub(crate) fn parsed(args: &str) -> Value {
    serde_json::from_str(args).unwrap_or(Value::Null)
}

/// 照参数估：既不是写入、也不是编辑的样子的，是 `None`。
pub(crate) fn from_args(args: &Value) -> Option<Diff> {
    if let Ok(write) = Write::deserialize(args) {
        return Some(estimated(count("", &write.content)));
    }
    let edit = Edit::deserialize(args).ok()?;
    let (added, removed) = edit
        .edits
        .iter()
        .map(|piece| count(&piece.old_string, &piece.new_string))
        .fold((0, 0), |(a, r), (x, y)| (a + x, r + y));
    Some(estimated((added, removed)))
}

fn estimated((added, removed): (u64, u64)) -> Diff {
    Diff {
        added,
        removed,
        estimated: true,
    }
}

/// 两份按行比：加了几行、删了几行。
fn count(before: &str, after: &str) -> (u64, u64) {
    TextDiff::from_lines(before, after)
        .iter_all_changes()
        .fold((0, 0), |(a, r), change| match change.tag() {
            ChangeTag::Insert => (a + 1, r),
            ChangeTag::Delete => (a, r + 1),
            ChangeTag::Equal => (a, r),
        })
}
