//! 一步工具碰了什么（施工 9-8 三补、四补，从 `entry.rs` 挪出来）：改了的文件、用了的技能。

use serde::Serialize;

/// 用了的技能（施工 9-8 四补）：名字，给人看的名字照连接的语言（没写的照名字）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillUsed {
    /// 技能的名字。
    pub name: String,
    /// 给人看的名字。
    pub title: String,
}

/// 一步改了的一个文件（施工 9-8 三补）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Touched {
    /// 换成真实位置以后的绝对路径。
    pub path: String,
    /// 改了还是移进了回收站。
    pub action: TouchKind,
}

/// 文件怎么了。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchKind {
    /// 新建：改之前没有这个文件（`file.changed` 没有 `before`）。
    Created,
    /// 覆盖、编辑已有的（`file.changed` 有 `before`）。
    Changed,
    /// 移进回收站（`file.trashed`）。
    Trashed,
}
