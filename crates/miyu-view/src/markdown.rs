//! 会话导出成 Markdown（施工 SE-1 中下上、中下中，设计 `58-会话导出和导入.md` 第一节，调研
//! `docs/reviews/2026-09-30-会话导出调研.md` 第四节第 1、3 条，第五节第 2、3、6 条）：一个不连线的头，吃视图条目、吐 Markdown。
//! 界面上有的导出里才有：挑哪些、显示名、一句话摘要、出错说明都照条目；条目里没有的原文（切过的工具结果、压缩的摘要、
//! 脚本里调的工具、她现在的上下文）由读日志的一方照事件记进 [`Sources`]。
//!
//! 纯函数：不碰磁盘、时钟。三个预设（[`Options::READ`]、[`Options::AGENT`]、[`Options::CHAT`]，`markdown/options.rs`），
//! 写了的开关盖过预设：
//!
//! - 开头（`markdown/head.rs`）：`read` 标题、时间段、模型、token 合计；`chat` 标题、时间段、token 估算；`agent` 一小段英文
//!   事实。范围从中间切开的，`read`、`chat` 写一句省掉了几轮，`agent` 的 `context` 把检查点摘要放最前（`markdown/select.rs`）。
//! - 角色的小标题一律英文（`User`、`Assistant`、`Tool`、`Subagent`）；`read` 的显示名、旁白照连接的语言（`core/export/…`），
//!   `agent`、`chat` 写工具的真名。
//! - 思考折在 `<details>` 里（`agent`、`chat` 默认不带）；工具 `read` 写显示名和摘要、输出照她当时看到的，`agent` 写真名、
//!   参数原文、结果截到头尾约 4000 字，`chat` 一行（`markdown/steps.rs`、`raw.rs`、`line.rs`）；要人确认的那一次、脚本里调的
//!   工具、工具上挂的提问照各自的写法（`markdown/extras.rs`）；图片、附件写占位；子代理 `read`、`agent` 写交代和回报，
//!   `chat` 只写回报；压缩照时间线放一条旁白，摘要折在下面。
//! - 不进的：撤掉的轮（`hidden`）、撤回的排队消息、撤销说明；注入的上下文、策略、系统提示词本来就不是条目。
//! - 正文原样；没合上的代码块替它合上，代码块的围栏比里面最长的一串反引号长。家目录写成 `~`；配了的密钥换成
//!   `[redacted: <名字>]`（`markdown/scrub.rs`）。

mod extras;
mod head;
mod line;
mod notices;
mod options;
mod raw;
mod scrub;
mod select;
mod sources;
mod steps;
mod text;

use std::collections::BTreeMap;

use miyu_kernel::estimate;
use miyu_kernel::origin::By;
use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::entry::{Attachment, Body, Entry, User};
use crate::words::{Texts, Words, say};
pub use head::AGENT_NOTE;
pub use options::{Options, Purpose, Range, Tools};
pub use sources::{Context, Sources};
use text::{Out, prose};

/// 给人看的字的编号前缀：都在内核那一份的 `export/` 下面。
const KEYS: &str = "core/export";

/// 会话本身的几样：开头写的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct About {
    /// 标题；没起标题的照第一句，都没有的是空的（写成「未命名会话」）。
    pub title: String,
    /// 造的时刻。
    pub from: Timestamp,
    /// 最近一次动静。
    pub to: Timestamp,
    /// 这个会话自己的模型调用一共用了多少 token（同会话列表的 `tokens`）。
    pub tokens: u64,
    /// 工作目录（同会话列表的 `cwd`）：`agent` 的开头写。
    pub cwd: Option<String>,
}

/// 渲染要的全部。
pub struct Document<'a> {
    /// 条目，照显示的先后（同 `view.page` 的 `view: true`）。
    pub entries: &'a [Entry],
    /// 会话本身的几样。
    pub about: &'a About,
    /// 条目里没有的原文。
    pub sources: &'a Sources,
    /// 给人看的字（连接的语言一份、英文一份）、工具的分类、家目录：同投影用的那一套。
    pub texts: &'a Texts,
    /// 开关。
    pub options: Options,
    /// 时间段照哪个时区写。
    pub offset: UtcOffset,
    /// 配了的密钥：名字、值。
    pub secrets: &'a [(&'a str, &'a str)],
}

/// 渲染好的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// Markdown。
    pub text: String,
    /// 隐去了哪几个密钥、各几处，照名字；一处都没有的不列。
    pub redacted: BTreeMap<String, usize>,
}

/// 照 `doc` 渲染：见模块的说明。
#[must_use]
pub fn render(doc: &Document<'_>) -> Rendered {
    let say = Say {
        local: doc.texts.local.as_ref(),
        english: doc.texts.english.as_ref(),
    };
    let picked = select::select(doc);
    let mut out = Out::default();
    head::write(&mut out, doc, &picked, &say);
    let mut speaker = Speaker::Nobody;
    for entry in picked.entries {
        speaker = match &entry.body {
            Body::User(user) => {
                write_user(&mut out, user, &say);
                Speaker::User
            }
            Body::Reply(reply) if reply.text.trim().is_empty() => speaker,
            Body::Reply(reply) => {
                assistant(&mut out, speaker);
                out.block(&prose(&reply.text, false));
                Speaker::Assistant
            }
            Body::Media(media) => {
                assistant(&mut out, speaker);
                let about = media.alt.as_deref().or(media.image.name.as_deref());
                out.block(&placeholder(&media.image, about, &say));
                Speaker::Assistant
            }
            Body::Thought(thought) if !doc.options.thinking || thought.text.trim().is_empty() => {
                speaker
            }
            Body::Thought(thought) => {
                assistant(&mut out, speaker);
                steps::thought(&mut out, thought, &say);
                Speaker::Assistant
            }
            Body::Tool(tool) => {
                assistant(&mut out, speaker);
                steps::tool(&mut out, tool, doc, &say);
                Speaker::Assistant
            }
            Body::Group(_) => speaker,
            Body::End(end) => notices::end(&mut out, end, &say).unwrap_or(speaker),
            Body::Notice(notice) => {
                notices::notice(&mut out, entry, notice, doc, &say).unwrap_or(speaker)
            }
        };
    }
    let (text, redacted) = scrub::redact(&out.finish(), doc.secrets);
    Rendered {
        text: estimated(scrub::tilde(&text, doc.texts.home.as_deref())),
        redacted,
    }
}

/// `chat` 开头的 token 估算换成数：同回应的 `tokens`（整份的 UTF-8 字节数除以 4），数本身也算在里面，算到不再变为止。
fn estimated(text: String) -> String {
    if !text.contains(head::ESTIMATE) {
        return text;
    }
    let fill = |n: u64| text.replacen(head::ESTIMATE, &n.to_string(), 1);
    let mut n = estimate::text(&fill(0));
    for _ in 0..8 {
        let again = estimate::text(&fill(n));
        if again == n {
            break;
        }
        n = again;
    }
    fill(n)
}

/// 上一块是谁的：她接着说的不再起小标题。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Speaker {
    /// 开头、旁白以后。
    Nobody,
    /// 一条消息。
    User,
    /// 她。
    Assistant,
    /// 子代理的回报。
    Subagent,
}

/// 她开口：上一块不是她的，先起小标题。
fn assistant(out: &mut Out, speaker: Speaker) {
    if speaker != Speaker::Assistant {
        out.block("## Assistant");
    }
}

/// 一条消息：小标题照来处，正文原样，附件写占位。
fn write_user(out: &mut Out, user: &User, say: &Say<'_>) {
    out.block(&format!("## {}", who(user.from.as_ref())));
    if !user.text.trim().is_empty() {
        out.block(&prose(&user.text, false));
    }
    let attached: Vec<String> = user
        .attachments
        .iter()
        .map(|attachment| {
            let name = attachment.name.as_deref().or(attachment.path.as_deref());
            format!("- {}", placeholder(attachment, name, say))
        })
        .collect();
    if !attached.is_empty() {
        out.block(&attached.join("\n"));
    }
}

/// 消息的小标题：人在本机说的、主人在平台私聊里说的是 `User`，别处来的写明来处。
fn who(from: Option<&By>) -> String {
    match from {
        Some(By::External(external)) => format!("User ({})", external.id),
        Some(By::Session(session)) => format!("Session ({})", short(session.id.as_str())),
        Some(By::Module(module)) => format!("Module ({})", module.id),
        Some(By::Harness(harness)) => format!("Harness ({})", harness.name),
        Some(By::Schedule(_)) => "Schedule".to_string(),
        Some(By::Kernel) => "Kernel".to_string(),
        _ => "User".to_string(),
    }
}

/// 会话编号的前 8 位（同头的短编号）。
fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

/// 图片、附件的占位：名字（`about`）、图的宽高，都没有的只写是什么。
fn placeholder(attachment: &Attachment, about: Option<&str>, say: &Say<'_>) -> String {
    let kind = match attachment.kind.as_str() {
        "image" => "image",
        _ => "file",
    };
    let mut parts: Vec<String> = about
        .map(text::one_line)
        .filter(|about| !about.is_empty())
        .into_iter()
        .collect();
    if let (Some(width), Some(height)) = (attachment.width, attachment.height) {
        parts.push(format!("{width}×{height}"));
    }
    match parts.is_empty() {
        true => say.say(&format!("{kind}/bare"), &[]),
        false => say.say(kind, &[("about", parts.join(", "))]),
    }
}

/// 给人看的字：先照连接的语言，没有的照英文，都没有的写编号（资源装坏了也不至于空着）。
pub(crate) struct Say<'a> {
    local: &'a dyn Words,
    english: &'a dyn Words,
}

impl Say<'_> {
    /// `core/export/<key>` 照字段说一句。
    fn say(&self, key: &str, fields: &[(&str, String)]) -> String {
        let key = format!("{KEYS}/{key}");
        say(self.local, &key, fields)
            .or_else(|| say(self.english, &key, fields))
            .unwrap_or(key)
    }

    /// 内核那一份里别处的编号（`view/…`）照字段说一句；没有的是空的。
    fn view(&self, key: &str, fields: &[(&str, String)]) -> Option<String> {
        say(self.local, key, fields).or_else(|| say(self.english, key, fields))
    }

    /// 数是 `count` 的一句：是 1 的编号多接 `/one`。
    fn count(&self, key: &str, count: u64, fields: &[(&str, String)]) -> String {
        let key = match count {
            1 => format!("{key}/one"),
            _ => key.to_string(),
        };
        let mut all = vec![("count", count.to_string())];
        all.extend(fields.iter().cloned());
        self.say(&key, &all)
    }
}
