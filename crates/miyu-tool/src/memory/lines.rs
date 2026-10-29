//! 一条记忆给她看的那一行（施工 R-3 中 `memory_search/memory.txt`；E-4 下加上在哪儿发生的）：`memory_search`、常驻那一块、
//! 联想、抽取里这一段以后忘掉的那几行、整理和写关键词的请求都照这一份写，写法一样。
//!
//! 在哪儿接在日期后面（`m12 episode 2026-10-21 in QQ group 「周末摄影」: …`）：记下那一刻有群名的（`place`，桥只在群里交）
//! 写群名；没有群名的照场所编号（`qq:group:…`、`qq:private:…`）写是哪个平台的群、私聊，别的形状的只写平台；主人那边记的、
//! 本机的没有场所，什么都不加。群名在这里原样换进那一小段（`fill`，不转义），整行再照模板的规矩转义一次：群名里的换行、
//! 引号、尖括号和正文一样转掉，那一小段的字里没有会被转的。

use std::collections::BTreeMap;
use std::path::Path;

use miyu_kernel::id::VenueId;
use miyu_kernel::template::Template;

use crate::load::{self, LoadError};

/// 一行要的几样。
#[derive(Debug, Clone, Copy)]
pub struct Line<'a> {
    /// 编号（`m12`）。
    pub id: &'a str,
    /// 类，认识的人那一类带着是谁（`miyu_recall::shown_class`）。
    pub class: &'a str,
    /// 记下的日期，照会话的时区。
    pub date: &'a str,
    /// 正文：换行怎么处理照调的一方原来的写法。
    pub text: &'a str,
    /// 在哪个场所说的；主人那边、本机的没有。
    pub venue: Option<&'a VenueId>,
    /// 记下那一刻的群名（施工 E-4 上）；没有的没有。
    pub place: Option<&'a str>,
    /// 作废了的（`memory_search` 带 `forgotten` 时）。
    pub retired: bool,
}

/// 一行的字（`resources/software/memory/memory_search/`，登记簿）。
#[derive(Debug, Clone)]
pub struct MemoryLines {
    line: Template,
    retired: Template,
    group: Template,
    group_unnamed: Template,
    private: Template,
    other: Template,
}

impl MemoryLines {
    /// 从资源目录 `resources` 里软件包 `package` 的 `memory_search/` 读：`memory.txt`、`retired.txt`（`{id}`、`{class}`、
    /// `{date}`、`{where}`、`{text}`），在哪儿的四句 `where-group.txt`（`{platform}`、`{place}`）、`where-group-unnamed.txt`、
    /// `where-private.txt`、`where-other.txt`（`{platform}`）。
    ///
    /// # Errors
    ///
    /// 哪一份读不出来、写法不对、要了别的字段。
    pub fn load(resources: &Path, package: &str) -> Result<MemoryLines, LoadError> {
        let text = |name: &str, fields: &[&str]| {
            load::text(resources, package, "memory_search", name, fields)
        };
        let line = ["id", "class", "date", "where", "text"];
        Ok(MemoryLines {
            line: text("memory", &line)?,
            retired: text("retired", &line)?,
            group: text("where-group", &["platform", "place"])?,
            group_unnamed: text("where-group-unnamed", &["platform"])?,
            private: text("where-private", &["platform"])?,
            other: text("where-other", &["platform"])?,
        })
    }

    /// 写一行，同 [`load::say`]：行尾照模板带着，调的一方照原来的写法去。
    pub fn say(&self, line: &Line<'_>) -> String {
        let place = self.place(line.venue, line.place);
        let template = if line.retired {
            &self.retired
        } else {
            &self.line
        };
        load::say(
            template,
            &[
                ("id", line.id),
                ("class", line.class),
                ("date", line.date),
                ("where", &place),
                ("text", line.text),
            ],
        )
    }

    /// 接在日期后面的那一段，前面带一个空格（`" in QQ group 「周末摄影」"`）；没有场所的是空的。没转义：整行写的时候转。
    ///
    /// # Panics
    ///
    /// 实际不会：造的时候试换过，字段都有。
    pub fn place(&self, venue: Option<&VenueId>, place: Option<&str>) -> String {
        let Some(venue) = venue else {
            return String::new();
        };
        // 照右边认（施工 O-63）：号里没有 `:`，最后一段是号、倒数第二段是种类，第一段是平台，中间多出来的（机器人账号，
        // `qq:<机器人>:group:<群号>`）不看。只有两段的照旧看第二段。
        let parts: Vec<&str> = venue.as_str().split(':').collect();
        let platform = platform(parts.first().copied().unwrap_or_default());
        let kind = match parts.len() {
            0..=2 => parts.get(1).copied(),
            n => parts.get(n - 2).copied(),
        };
        let raw = |template: &Template, fields: &[(&str, &str)]| {
            let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
            template
                .fill(&fields, str::to_string)
                .expect("造的时候试换过，字段都有")
                .trim_end()
                .to_string()
        };
        let phrase = match (place, kind) {
            (Some(place), _) => raw(&self.group, &[("platform", platform), ("place", place)]),
            (None, Some("group")) => raw(&self.group_unnamed, &[("platform", platform)]),
            (None, Some("private")) => raw(&self.private, &[("platform", platform)]),
            (None, _) => raw(&self.other, &[("platform", platform)]),
        };
        format!(" {phrase}")
    }
}

/// 场所编号头一截（`qq`）给她看的平台名：认识的几个写惯用的大小写，别的原样。
fn platform(prefix: &str) -> &str {
    match prefix {
        "qq" => "QQ",
        "wechat" => "WeChat",
        "telegram" => "Telegram",
        "discord" => "Discord",
        other => other,
    }
}

#[cfg(test)]
mod tests;
