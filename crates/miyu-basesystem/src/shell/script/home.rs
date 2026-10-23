//! 会话属主家目录的那几处（施工 Y-11 补，`docs/blueprint/skills.md`「脚本」第 3 条）：`pictures/`、`videos/`、`documents/`、
//! `downloads/`（[`miyu_store::root::USER_DIRS`]），各交一个环境变量，同缓存目录、状态目录那样跑之前建好、交真实位置、放进脚本
//! 的沙盒写名单。只给本机的会话：场所会话（群、陌生人私聊）没有一个人的家，给了就是几个群共用一处（施工 X-44 补刚把状态目录
//! 照场所分开）；那里的脚本照旧用按场所分的缓存目录。不知道在哪个场所的也不给。`shell` 不另加（施工 Y-11 上定的）。

use std::path::PathBuf;

use miyu_store::root::{USER_DIRS, user_dir};
use miyu_tool::Call;

/// 每一处交给脚本的环境变量，照 [`USER_DIRS`] 的先后。
const VARS: [&str; USER_DIRS.len()] = [
    "MIYU_PICTURES_DIR",
    "MIYU_VIDEOS_DIR",
    "MIYU_DOCUMENTS_DIR",
    "MIYU_DOWNLOADS_DIR",
];

/// 本机的场所。
const LOCAL: &str = "local";

/// 这一次该交的几处：环境变量和位置（还没建）。场所会话、不知道场所的、会话外面的（没有数据根、属主）没有。
pub(super) fn wanted(call: &Call) -> Vec<(&'static str, PathBuf)> {
    let local = call
        .caller
        .as_ref()
        .is_some_and(|caller| caller.venue.as_str() == LOCAL);
    let (Some(root), Some(ids), true) = (call.data_root.as_deref(), call.ids.as_ref(), local)
    else {
        return Vec::new();
    };
    VARS.iter()
        .zip(USER_DIRS)
        .map(|(var, name)| (*var, user_dir(root, &ids.owner, name)))
        .collect()
}
