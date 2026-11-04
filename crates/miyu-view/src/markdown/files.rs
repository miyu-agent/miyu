//! `assets: files`（施工 SE-1 中下下，调研第五节第 7 条「连图一起，导成一个文件夹，Markdown 里写相对路径」）：图片、附件
//! 不写占位，写成指向旁边那个文件夹里一个文件的相对链接；哪个 blob 叫什么名字记在 [`Files`] 里，写文件的一方照它把 blob 拷
//! 进文件夹。渲染器本身不碰磁盘：哪些 blob 取得到由写文件的一方先查好（[`Files::have`]），取不到的照占位写、记进
//! [`Files::missing`]。
//!
//! - 名字：附件的名字，没有的照路径的最后一截，都没有的 `image-<哈希前 8 位>.<类型>`、`file-…`；只留最后一截，Windows 不收的
//!   字（`<>:"/\|?*`、控制字符）换成 `_`，头尾的空格和点去掉，最多 80 个字（扩展名留着），`CON`、`NUL` 这类前面垫 `_`。
//! - 同一个 blob 只拷一份、用头一回起的名字；不同的 blob 撞了名（不分大小写：macOS、Windows 上是同一个文件）的，后来的在
//!   扩展名前接 `-2`、`-3`。
//! - 链接：`<文件夹>/<名字>`，两截里的空格、括号、尖括号、方括号、`#`、`?`、`%`、反斜杠、反引号、引号、控制字符照 URL 写成
//!   `%XX`；图写 `![说明](链接)`，别的附件写 `[说明](链接)`，说明是名字、宽高，什么都没有的照文件名。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::id::ContentHash;

use crate::entry::{Body, Entry};

/// 名字最多几个字（照 Unicode 的字数）。
const LONGEST: usize = 80;

/// 一次导出的附件文件夹：叫什么、哪些 blob 取得到、取了哪些、起了什么名字。主会话和附在后面的子会话共用一份。
#[derive(Debug, Default)]
pub struct Files {
    /// 文件夹的名字（Markdown 文件名去掉扩展名接 `_files`）：链接写成它开头的相对路径。
    dir: String,
    /// 写文件的一方查过、取得到的 blob。
    present: BTreeSet<ContentHash>,
    /// 渲染时记下的。
    named: RefCell<Named>,
}

/// 渲染时记下的：起好的名字、用掉的名字、取不到的。
#[derive(Debug, Default)]
struct Named {
    /// blob 到文件夹里的文件名。
    names: BTreeMap<ContentHash, String>,
    /// 用掉的名字，小写。
    taken: BTreeSet<String>,
    /// 要写、取不到的。
    missing: BTreeSet<ContentHash>,
}

impl Files {
    /// 一个叫 `dir` 的文件夹，还什么都没有。
    #[must_use]
    pub fn new(dir: impl Into<String>) -> Files {
        Files {
            dir: dir.into(),
            ..Files::default()
        }
    }

    /// 这几个 blob 取得到：渲染前由写文件的一方查好（每个会话渲染前查它的那几个，[`wanted`]）。
    pub fn have(&mut self, present: impl IntoIterator<Item = ContentHash>) {
        self.present.extend(present);
    }

    /// 起了名字、要拷进文件夹的：blob 到文件名。
    #[must_use]
    pub fn named(&self) -> BTreeMap<ContentHash, String> {
        self.named.borrow().names.clone()
    }

    /// 要写进来、取不到的 blob：照占位写了。
    #[must_use]
    pub fn missing(&self) -> BTreeSet<ContentHash> {
        self.named.borrow().missing.clone()
    }

    /// 一张图、一个附件写成链接：`about` 是说明（名字、宽高），`name` 是起名用的（附件的名字、路径）；取不到的交回 `None`。
    pub(super) fn link(
        &self,
        blob: &ContentHash,
        image: bool,
        about: Option<String>,
        (name, media_type): (Option<&str>, &str),
    ) -> Option<String> {
        if !self.present.contains(blob) {
            self.named.borrow_mut().missing.insert(blob.clone());
            return None;
        }
        let file = self.name(blob, image, name, media_type);
        let path = format!("{}/{}", url(&self.dir), url(&file));
        let about = escape(&about.unwrap_or(file));
        Some(match image {
            true => format!("![{about}]({path})"),
            false => format!("[{about}]({path})"),
        })
    }

    /// `blob` 在文件夹里叫什么：起过的照旧，没起过的照规矩起一个不撞的。
    fn name(
        &self,
        blob: &ContentHash,
        image: bool,
        name: Option<&str>,
        media_type: &str,
    ) -> String {
        let mut named = self.named.borrow_mut();
        if let Some(name) = named.names.get(blob) {
            return name.clone();
        }
        let wanted = name.map(clean).filter(|name| !name.is_empty());
        let wanted = wanted.unwrap_or_else(|| {
            let kind = if image { "image" } else { "file" };
            let hex: String = blob.hex().chars().take(8).collect();
            format!("{kind}-{hex}.{}", extension(media_type))
        });
        let (stem, dot) = match wanted.rfind('.').filter(|at| *at > 0) {
            Some(at) => (&wanted[..at], &wanted[at..]),
            None => (wanted.as_str(), ""),
        };
        let mut file = wanted.clone();
        let mut n = 2;
        while named.taken.contains(&file.to_lowercase()) {
            file = format!("{stem}-{n}{dot}");
            n += 1;
        }
        named.taken.insert(file.to_lowercase());
        named.names.insert(blob.clone(), file.clone());
        file
    }
}

/// 要渲染的条目里写到的 blob：人附的图片、附件，她发出去的图，工具结果里的图；撤掉的、撤回的不算。写文件的一方照它查哪些
/// 取得到。
#[must_use]
pub fn wanted(entries: &[Entry]) -> BTreeSet<ContentHash> {
    let mut wanted = BTreeSet::new();
    for entry in entries.iter().filter(|entry| !entry.hidden) {
        match &entry.body {
            Body::User(user) if !user.withdrawn => {
                wanted.extend(user.attachments.iter().map(|a| a.blob.clone()));
            }
            Body::Media(media) => {
                wanted.insert(media.image.blob.clone());
            }
            Body::Tool(tool) => wanted.extend(tool.images.iter().map(|p| p.blob.clone())),
            _ => {}
        }
    }
    wanted
}

/// 名字收拾成三个平台都收的一截：见模块的说明。
fn clean(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let swapped: String = last
        .chars()
        .map(|c| match c.is_control() || "<>:\"/\\|?*".contains(c) {
            true => '_',
            false => c,
        })
        .collect();
    let trimmed = swapped.trim_matches([' ', '.']);
    let (stem, dot) = match trimmed.rfind('.').filter(|at| *at > 0) {
        Some(at) if trimmed.len() - at <= 10 => (&trimmed[..at], &trimmed[at..]),
        _ => (trimmed, ""),
    };
    let room = LONGEST.saturating_sub(dot.chars().count());
    let stem: String = stem.chars().take(room).collect();
    let stem = stem.trim_end_matches([' ', '.']);
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let upper = stem.to_ascii_uppercase();
    let numbered = (upper.starts_with("COM") || upper.starts_with("LPT"))
        && upper.len() == 4
        && upper.as_bytes()[3].is_ascii_digit();
    match reserved.contains(&upper.as_str()) || numbered {
        true => format!("_{stem}{dot}"),
        false => format!("{stem}{dot}"),
    }
}

/// 类型的扩展名：`image/png` 是 `png`，`image/svg+xml` 是 `svg`；认不出的 `bin`。
fn extension(media_type: &str) -> String {
    let sub = media_type
        .split('/')
        .nth(1)
        .and_then(|sub| sub.split(['+', ';']).next())
        .unwrap_or_default()
        .trim();
    match !sub.is_empty() && sub.len() <= 8 && sub.chars().all(|c| c.is_ascii_alphanumeric()) {
        true => sub.to_ascii_lowercase(),
        false => "bin".to_string(),
    }
}

/// 链接里的一截：会弄断 Markdown 链接、会被当成 URL 记号的字写成 `%XX`，别的（中文这些）照原样。
fn url(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for c in part.chars() {
        match c.is_control() || " ()<>[]#?%\\`\"'".contains(c) {
            true => {
                let mut bytes = [0; 4];
                for byte in c.encode_utf8(&mut bytes).bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
            false => out.push(c),
        }
    }
    out
}

/// 链接的说明：一行，`\`、`[`、`]` 前面垫反斜杠。
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in super::text::one_line(text).chars() {
        if matches!(c, '\\' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests;
