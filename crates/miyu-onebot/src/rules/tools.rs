//! 桥的工具（施工 O-26，`onebot.md` 第一条「提供者和不说话」，「场所规则和出厂数据」第 1、2 条；O-31 加撤回、禁言、戳一戳，
//! 「平台工具（一）」；O-33 加 `fetch_media`，「平台工具（二）」）：桥答得了哪几件、各自的访问类别和给哪种会话写在 [`TOOLS`] 这张表里（跟着答它的代码，「施工时定的」
//! 第 138 条）；说明照包文件夹的 `data/tools/<名字>.json` 读（`miyu_client::tooltext::load::spec_in`，`providers.md`「给模型看的字」；施工 S-6
//! 上从资源目录的 `software/onebot/` 挪来），答的话照 `data/tool-results/` 读（`miyu_client::tooltext::load::text_in`，试换过字段）。都是给模型看的字，登记在 `26-提示词.md` 第十节；
//! 桥起来时和别的出厂数据一起读，写坏了是打包的错，起不来。
//!
//! 交出去的：`provide` 的参数（[`Tools::provided`]），读的一头当场答的 `tool.call` 的结果（[`Tools::call`]），交给跟核心的那一头
//! 答的几件（[`Tools::routed`]）和它们答的话（[`Tools::answer`]，施工 O-31）。

use std::collections::HashMap;
use std::path::Path;

use miyu_chat::{Problem, Source};
use miyu_client::protocol::Template;
use miyu_client::settings::problem::Code;
use miyu_client::tooltext::load::{self, LoadError};
use miyu_client::tooltext::{Access, Spec};
use serde_json::{Value, json};

use super::files;
use super::names::{self, Names};

/// 「这一轮不说话」（「提供者和不说话」第 3 条）：她的回复里有这件的调用块，这一轮就不发了（`core/route/quiet.rs`）。
pub(crate) const SKIP_REPLY: &str = "skip_reply";

/// 撤回（施工 O-31，「平台工具（一）」）：撤叫她做的那条引用的那一条。
pub(crate) const RECALL: &str = "recall";

/// 禁言（施工 O-31）：只给群。
pub(crate) const MUTE: &str = "mute";

/// 戳一戳（施工 O-31）。
pub(crate) const POKE: &str = "poke";

/// 取图、视频、文件（施工 O-33，「平台工具（二）」）：私聊、群都给（施工 O-35 起私聊也给）。
pub(crate) const FETCH_MEDIA: &str = "fetch_media";

/// 私聊、群都给。
const VENUES: &[&str] = &["private", "group"];

/// 桥答得了的工具：名字、访问类别、给哪种会话（`providers.md` 的 `venues`）。`skip_reply` 只读：不碰主机、不往外发（O-26
/// 施工单「要定的」第 4 条）；平台工具（一）在平台上留下后果，是在场所里做的事（`venue`）：权限策略在场所会话里放行、本机的
/// 会话里拒绝，谁能叫、能动谁由桥自己挡（`providers.md`「在场所里做的事」，「施工时定的」第 177 条）。本机的会话都没有。
/// `fetch_media` 也是在场所里做的事（施工 O-33：去平台取东西、写进这个会话自己的工作区）；O-33 只给群，O-35 起私聊里带了东西
/// 的那条也写出消息编号，私聊也给。
const TOOLS: [(&str, Access, &[&str]); 5] = [
    (SKIP_REPLY, Access::Read, VENUES),
    (RECALL, Access::Venue, VENUES),
    (MUTE, Access::Venue, &["group"]),
    (POKE, Access::Venue, VENUES),
    (FETCH_MEDIA, Access::Venue, VENUES),
];

/// 答的那几句在包文件夹的 `data/` 下的哪个目录里（照核心的 `core/tool-results/`）。
const RESULTS: &str = "tool-results";

/// 答 `skip_reply` 的那一句：不要字段。
const SKIPPED: &str = "skipped";

/// 不认识的工具答的那一句：字段只认 `name`（工具名）。
const UNKNOWN: &str = "unknown";

/// 平台工具（一）答的话（施工 O-31，「平台工具（一）」第 5 条）：名字（`tool-results/<名字>.txt`）和认的字段；`saved` 以后是
/// `fetch_media` 的（施工 O-33，「平台工具（二）」第 6 条），`failed`、`unreachable`、`unanswered` 共用。
const ANSWERS: [(&str, &[&str]); 20] = [
    ("recalled", &[]),
    ("muted", &["who", "duration"]),
    ("unmuted", &["who"]),
    ("poked", &["who"]),
    ("not-allowed", &[]),
    ("no-quote", &[]),
    ("no-target", &[]),
    ("many-targets", &[]),
    ("protected", &["who"]),
    ("bad-seconds", &[]),
    ("failed", &["detail"]),
    ("unreachable", &[]),
    ("unanswered", &[]),
    ("saved", &["path"]),
    ("not-found", &[]),
    ("no-item", &["count"]),
    ("not-fetchable", &[]),
    ("unfetched", &["detail"]),
    ("too-many", &["count"]),
    ("not-saved", &["detail"]),
];

/// 读好、查过的桥的工具。
#[derive(Debug)]
pub struct Tools {
    /// 每件的规格和给哪种会话，照 [`TOOLS`] 的先后。
    specs: Vec<(Spec, &'static [&'static str])>,
    /// 答 `skip_reply` 的那一句（`tool-results/skipped.txt`）。
    skipped: String,
    /// 不认识的工具答的那一句（`tool-results/unknown.txt`）。
    unknown: Template,
    /// 平台工具（一）答的话，照名字（[`ANSWERS`]）。
    answers: HashMap<&'static str, Template>,
    /// 每件给人看的名字（施工 S-6 上）：照语言，`provide` 时带上（`display_name`）。
    names: Names,
}

impl Tools {
    /// `provide` 的参数（「提供者和不说话」第 1 条）：每件 `{name, description, input_schema, access, venues}`，有给人看的名字
    /// 的再带 `display_name`（`{语言: 名字}`，施工 S-6 上，[`names`]），写了名字后面跟哪一个参数的再带 `subject`；不写 `timeout_ms`（照核心的一分钟：这几件当场答，平台工具
    /// 等 NapCat 也在 `call_timeout_seconds` 里）。
    pub(crate) fn provided(&self) -> Value {
        let tools: Vec<Value> = self
            .specs
            .iter()
            .map(|(spec, venues)| {
                let mut tool = json!({
                    "name": spec.name,
                    "description": spec.description,
                    "input_schema": spec.parameters,
                    "access": spec.access,
                    "venues": venues,
                });
                if let Some(face) = self.names.get(&spec.name) {
                    tool["display_name"] = json!(face.names);
                    if let Some(subject) = &face.subject {
                        tool["subject"] = json!(subject);
                    }
                }
                tool
            })
            .collect();
        json!({"tools": tools})
    }

    /// 工具 `tool` 是不是交给跟核心的那一头答的（施工 O-31，「平台工具（一）」第 2 条）：撤回、禁言、戳一戳要投影、群成员的
    /// 缓存和机器人号的连接；`fetch_media`（施工 O-33）要会话发到哪、这一轮取了几次。
    pub(crate) fn routed(&self, tool: &str) -> bool {
        [RECALL, MUTE, POKE, FETCH_MEDIA].contains(&tool)
    }

    /// 读的一头当场答的 `tool.call` 的结果（`tool.call` 回应的 `result`，「提供者和不说话」第 2、3 条）：`skip_reply` 答出厂的
    /// 那一句，不算出错，带 `end_turn: true`（施工 O-38，施工时定的第 238 条：核心这一步齐了不再请求，这一轮里又叫她的走新的
    /// 一轮）；不认识的答 `unknown.txt` 填上那个名字，算出错。
    pub(crate) fn call(&self, tool: &str) -> Value {
        if tool == SKIP_REPLY {
            let mut answer = result(&self.skipped, false);
            answer["end_turn"] = Value::Bool(true);
            return answer;
        }
        // 起来时试换过 `name`：换得出。
        result(&load::say(&self.unknown, &[("name", tool)]), true)
    }

    /// 平台工具（一）答的话 `name`（[`ANSWERS`] 里的一个），字段照 `fields` 换，`error` 照写（施工 O-31，「平台工具（一）」
    /// 第 5 条）。
    ///
    /// # Panics
    ///
    /// `name` 不在 [`ANSWERS`] 里、少了它要的字段：是 bug（名字、字段都写在调的一方的代码里，测试走得到）。
    pub(crate) fn answer(&self, name: &str, fields: &[(&str, &str)], error: bool) -> Value {
        let template = self.answers.get(name).expect("答的话在 ANSWERS 里");
        result(&load::say(template, fields), error)
    }
}

/// 一段字的结果：内容块照内核的写法，`error` 照写。
fn result(text: &str, error: bool) -> Value {
    json!({"blocks": [{"type": "text", "text": text}], "error": error})
}

/// 读包文件夹的数据目录 `data` 里桥的工具：说明、答的话、给人看的名字（施工 S-6 上，`names`）。读不成、写坏了、要了别的字段的，每一份记一条 `bad_format`（文件照
/// `data/` 下的位置写，原话照 `miyu_client::tooltext::load` 说的），交回空的。
pub(super) fn load(data: &Path, problems: &mut Vec<Problem>) -> Option<Tools> {
    // 给人看的名字先读（施工 S-6 上）：读不懂的那一份先记一条问题，下面记问题的那一个要一直借着 `problems`。
    let named: Vec<&str> = TOOLS.iter().map(|(name, _, _)| *name).collect();
    let names = names::load(data, &named, problems);
    let mut trouble = |file: String, error: LoadError| {
        problems.push(files::whole(
            Code::BadFormat,
            Source::Factory,
            &file,
            Some(error.why),
        ));
    };
    let mut specs = Vec::new();
    for (name, access, venues) in &TOOLS {
        match load::spec_in(data, name, access.clone()) {
            Ok(spec) => specs.push((spec, *venues)),
            Err(error) => trouble(format!("tools/{name}.json"), error),
        }
    }
    let text = |name: &str, fields: &[&str]| {
        load::text_in(data, RESULTS, name, fields)
            .map_err(|error| (format!("{RESULTS}/{name}.txt"), error))
    };
    let skipped = text(SKIPPED, &[]).map(|template| load::say(&template, &[]));
    let unknown = text(UNKNOWN, &["name"]);
    let mut answers = HashMap::new();
    let mut broken = Vec::new();
    for (name, fields) in ANSWERS {
        match text(name, fields) {
            Ok(template) => {
                answers.insert(name, template);
            }
            Err(error) => broken.push(error),
        }
    }
    match (skipped, unknown) {
        (Ok(skipped), Ok(unknown)) if specs.len() == TOOLS.len() && broken.is_empty() => {
            Some(Tools {
                specs,
                skipped,
                unknown,
                answers,
                names,
            })
        }
        (skipped, unknown) => {
            let first = [skipped.err(), unknown.err()].into_iter().flatten();
            for (file, error) in first.chain(broken) {
                trouble(file, error);
            }
            None
        }
    }
}
