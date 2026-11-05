//! 会话导出成 Markdown（施工 SE-1 中下上、中下中、中下下，设计 `58-会话导出和导入.md` 第一节，调研
//! `docs/reviews/2026-09-30-会话导出调研.md` 第四节第 1、3 条，第五节第 2、3、6、7 条）：一个不连线的头，吃视图条目、吐 Markdown。
//! 界面上有的导出里才有：挑哪些、显示名、一句话摘要、出错说明都照条目；条目里没有的原文（切过的工具结果、压缩的摘要、
//! 脚本里调的工具、她现在的上下文、派出去的子代理）由读日志的一方照事件记进 [`Sources`]。
//!
//! 纯函数：不碰磁盘、时钟。三个预设（[`Options::READ`]、[`Options::AGENT`]、[`Options::CHAT`]，`markdown/options.rs`），
//! 写了的开关盖过预设：
//!
//! - 开头（`markdown/head.rs`）：`read` 标题、时间段、模型、token 合计；`chat` 标题、时间段、token 估算；`agent` 一小段英文
//!   事实。范围从中间切开的，`read`、`chat` 写一句省掉了几轮，`agent` 的 `context` 把检查点摘要放最前（`markdown/select.rs`）。
//! - 角色的小标题一律英文（`User`、`Assistant`、`Tool`、`Subagent`）；`read`、`chat` 的显示名、旁白照连接的语言
//!   （`core/export/…`），`agent` 的旁白、占位一律英文（`markdown/agent.rs` 的常量，施工 SE-1 中下下）；`agent`、`chat` 写
//!   工具的真名。
//! - 思考折在 `<details>` 里（`agent`、`chat` 默认不带）；工具 `read` 写显示名和摘要、输出照她当时看到的，`agent` 写真名、
//!   参数原文、结果截到头尾约 4000 字，`chat` 一行（`markdown/steps.rs`、`raw.rs`、`line.rs`）；要人确认的那一次、脚本里调的
//!   工具、工具上挂的提问照各自的写法（`markdown/extras.rs`）；图片、附件写占位，`assets: files` 写成指向旁边文件夹的相对
//!   链接（`markdown/files.rs`）；子代理 `read`、`agent` 写交代和回报，`chat` 只写回报；压缩照时间线放一条旁白，摘要折在下面。
//! - `children: append`：读日志的一方把渲染进去的那几步派的子会话（[`Draft::children`]）一个个照同一套开关渲染（[`draft`]，
//!   开头换成 `# Subagent <任务编号>: <标题>`），照任务编号接在后面，最后一起过一遍（[`finish`]）。
//! - 不进的：撤掉的轮（`hidden`）、撤回的排队消息、撤销说明；注入的上下文、策略、系统提示词本来就不是条目。
//! - 正文原样；没合上的代码块替它合上，代码块的围栏比里面最长的一串反引号长。家目录写成 `~`；配了的密钥换成
//!   `[redacted: <名字>]`（`markdown/scrub.rs`）。

mod agent;
mod extras;
mod files;
mod head;
mod line;
mod notices;
mod options;
mod raw;
mod scrub;
mod select;
mod shell;
mod sources;
mod steps;
mod text;

use std::collections::BTreeMap;

use miyu_kernel::estimate;
use miyu_kernel::id::{JobId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::entry::{Attachment, Body, Entry, User};
use crate::words::{Texts, Words, say};
pub use files::{Files, wanted as assets};
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
    /// 附在主会话后面的子会话（`children: append`）：开头换成子代理的小标题；主会话的是 `None`。
    pub child: Option<&'a Child>,
    /// `assets: files` 的附件文件夹：图片、附件写成指向它的相对链接；写占位的是 `None`。
    pub files: Option<&'a Files>,
}

/// 一个派出去的子代理：派它那一步的结果里记的（[`Sources::jobs`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Child {
    /// 任务编号：附在后面照它排（`j1`、`j1.1`、`j2`）。
    pub job: JobId,
    /// 子会话。
    pub session: SessionId,
    /// 派它时给的标题（`description`）。
    pub title: String,
}

/// 一个会话渲染好、还没最后过一遍的（[`draft`]）：几个接起来交给 [`finish`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// Markdown，密钥、家目录、token 估算还没换。
    pub text: String,
    /// 写进去的那几步派的子代理，照任务编号；同一个子会话只列一次。
    pub children: Vec<Child>,
}

/// 渲染好的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// Markdown。
    pub text: String,
    /// 隐去了哪几个密钥、各几处，照名字；一处都没有的不列。
    pub redacted: BTreeMap<String, usize>,
}

/// 照 `doc` 渲染一个会话、最后过一遍：见模块的说明。
#[must_use]
pub fn render(doc: &Document<'_>) -> Rendered {
    finish(&[draft(doc)], doc.secrets, doc.texts.home.as_deref())
}

/// 照 `doc` 渲染一个会话，还不换密钥、家目录、token 估算：主会话和附在后面的子会话各一份，接起来交给 [`finish`]。
#[must_use]
pub fn draft(doc: &Document<'_>) -> Draft {
    let say = Say {
        local: doc.texts.local.as_ref(),
        english: doc.texts.english.as_ref(),
        agent: doc.options.purpose == Purpose::Agent,
    };
    let picked = select::select(doc);
    let mut out = Out::default();
    head::write(&mut out, doc, &picked, &say);
    let mut speaker = Speaker::Nobody;
    let mut children: Vec<Child> = Vec::new();
    for entry in &picked.entries {
        speaker = match &entry.body {
            Body::User(user) => {
                write_user(&mut out, user, doc, &say);
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
                out.block(&asset(&media.image, about, doc, &say));
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
                if let Some((session, title)) =
                    tool.job.as_ref().and_then(|job| doc.sources.jobs.get(job))
                    && let Some(job) = &tool.job
                    && !children.iter().any(|child| child.session == *session)
                {
                    children.push(Child {
                        job: job.clone(),
                        session: session.clone(),
                        title: title.clone(),
                    });
                }
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
    children.sort_by(|a, b| a.job.cmp(&b.job));
    Draft {
        text: out.finish(),
        children,
    }
}

/// 接起来的几份最后过一遍：配了的密钥换掉、家目录写成 `~`、`chat` 的 token 估算换成数（整份的，连附在后面的）。
#[must_use]
pub fn finish(drafts: &[Draft], secrets: &[(&str, &str)], home: Option<&str>) -> Rendered {
    let whole: Vec<&str> = drafts.iter().map(|draft| draft.text.as_str()).collect();
    let (text, redacted) = scrub::redact(&whole.join("\n"), secrets);
    Rendered {
        text: estimated(scrub::tilde(&text, home)),
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

/// 一条消息：小标题照来处，正文原样，人用 `!` 跑的写成命令和输出（`markdown/shell.rs`），附件写占位。
fn write_user(out: &mut Out, user: &User, doc: &Document<'_>, say: &Say<'_>) {
    out.block(&format!("## {}", who(user.from.as_ref())));
    match &user.shell {
        Some(ran) => shell::write(out, &user.text, ran, doc, say),
        None if !user.text.trim().is_empty() => out.block(&prose(&user.text, false)),
        None => {}
    }
    let attached: Vec<String> = user
        .attachments
        .iter()
        .map(|attachment| {
            let name = attachment.name.as_deref().or(attachment.path.as_deref());
            format!("- {}", asset(attachment, name, doc, say))
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

/// 一张图、一个附件：`assets: files` 取得到的写成链接（[`Files`]），别的写占位。说明是名字（`about`）、图的宽高。
fn asset(
    attachment: &Attachment,
    about: Option<&str>,
    doc: &Document<'_>,
    say: &Say<'_>,
) -> String {
    let image = attachment.kind == "image";
    let size = attachment.width.zip(attachment.height);
    let name = attachment.name.as_deref().or(attachment.path.as_deref());
    let link = doc.files.and_then(|files| {
        let about = described(about, size);
        files.link(
            &attachment.blob,
            image,
            about,
            (name, &attachment.media_type),
        )
    });
    link.unwrap_or_else(|| placeholder(image, described(about, size), say))
}

/// 说明：名字、宽高，用 `, ` 隔开；都没有的是 `None`。
fn described(about: Option<&str>, size: Option<(u32, u32)>) -> Option<String> {
    let mut parts: Vec<String> = about
        .map(text::one_line)
        .filter(|about| !about.is_empty())
        .into_iter()
        .collect();
    if let Some((width, height)) = size {
        parts.push(format!("{width}×{height}"));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// 图片、附件的占位：有说明的写说明，没有的只写是什么。
fn placeholder(image: bool, about: Option<String>, say: &Say<'_>) -> String {
    let kind = if image { "image" } else { "file" };
    match about {
        Some(about) => say.say(kind, &[("about", about)]),
        None => say.say(&format!("{kind}/bare"), &[]),
    }
}

/// 给人看的字：先照连接的语言，没有的照英文，都没有的写编号（资源装坏了也不至于空着）。`agent` 预设不照语言：先照
/// `markdown/agent.rs` 的英文常量，没有的照英文那一份。
pub(crate) struct Say<'a> {
    local: &'a dyn Words,
    english: &'a dyn Words,
    /// `agent` 预设：一律英文。
    agent: bool,
}

impl Say<'_> {
    /// `core/export/<key>` 照字段说一句。
    fn say(&self, key: &str, fields: &[(&str, String)]) -> String {
        let key = format!("{KEYS}/{key}");
        self.view(&key, fields).unwrap_or(key)
    }

    /// 内核那一份里的编号（`core/view/…`、`core/export/…`）照字段说一句；没有的是空的。
    fn view(&self, key: &str, fields: &[(&str, String)]) -> Option<String> {
        match self.agent {
            true => agent::say(key, fields).or_else(|| say(self.english, key, fields)),
            false => say(self.local, key, fields).or_else(|| say(self.english, key, fields)),
        }
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
