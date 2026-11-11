//! 群里一行的行首（施工 FT-1 从 `group.rs` 分出来）：`[时:分] 发的人 [msg=编号]<记号>: `。拼的（[`head`]）和认的（[`starts`]）
//! 用同一份字。
//!
//! 认它是给内核截「模型编的下一轮」用的（`docs/blueprint/kernel/session.md`「截掉模型编的下一轮」）：模型在回答后面写出一个
//! 角色词，接着照这一行的样子编一条别人说的话，内核经组装器的 `record_head` 问到这里。

/// 发的人后面、平台编号前面的那一段。
const MSG_OPEN: &str = " [msg=";
/// 平台编号收住。
const MSG_CLOSE: char = ']';
/// 行首收住，后面是内容。
const SAYS: char = ':';

/// 一行的行首：`clock` 是钟点（`时:分`），`sender` 是发的人（转义过的名字和括号里的身份，她自己是 `[you]`），`msg` 是转义过的
/// 平台编号，`mark` 是编号后面可信的记号（撤回的），没有的是空的。后面直接接内容。
pub(super) fn head(clock: &str, sender: &str, msg: &str, mark: &str) -> String {
    format!("[{clock}] {sender}{MSG_OPEN}{msg}{MSG_CLOSE}{mark}{SAYS} ")
}

/// `text` 的头一行是不是照 [`head`] 写的：`[两位:两位] ` 开头，后面有 ` [msg=`，编号收住以后紧跟冒号。发的人写没写平台
/// 身份（`(id=…)`，跟着那一条的 `show_ids`）都认；带撤回记号的不认：编下一轮的不会编一条撤回了的。
pub(crate) fn starts(text: &str) -> bool {
    let line = text.lines().next().unwrap_or_default();
    let clock = line.as_bytes().get(..8).is_some_and(|head| {
        let digits = [1, 2, 4, 5].iter().all(|&at| head[at].is_ascii_digit());
        digits && head[0] == b'[' && head[3] == b':' && head[6] == b']' && head[7] == b' '
    });
    if !clock {
        return false;
    }
    line[8..]
        .split_once(MSG_OPEN)
        .and_then(|(_, msg)| msg.split_once(MSG_CLOSE))
        .is_some_and(|(_, rest)| rest.starts_with(SAYS))
}

#[cfg(test)]
mod tests;
