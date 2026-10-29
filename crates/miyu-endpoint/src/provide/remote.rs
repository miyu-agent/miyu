//! 提供者的一件工具（施工 O-2 上，`docs/blueprint/providers.md`「怎么走」第 3 条）：执行时照包查提供者表，没有连接的交回「暂时
//! 不可用」；有的，反向调用 `tool.call {session, call_id, tool, args, cwd, by, owner}`，照回应 `{blocks, error}` 交回结果。回应是
//! 错误的、写法不对的，算出错，原话写进结果；等着时连接断了，也是暂时不可用。到了登记时写的时限没回的，交回「没在 N 秒内答完」；
//! 超时、叫它停、掐掉（这个 future 被丢掉）时发一次 `tool.cancel {session, call_id}`，回应先到的不发（施工 O-2 下）。
//!
//! 交回的图、文件引用的 blob 是扩展经 `blob.put` 传进它连接的账号名下的（施工 Y-2中下），她照会话的属主读：回应到了先拷一份进属主名下，块原样不动；
//! 拷不成的（提供者名下也没有这个 blob、存不下）照暂时不可用（施工 O-2 三补，同 `session.send` 的附件，`attach::copy_over`）。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Said, Sent};
use miyu_kernel::id::ContentHash;
use miyu_kernel::template::Template;
use miyu_store::root::DataRoot;
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool, Venues};

use super::{Checked, Provided};
use crate::reverse::{Gone, Peer};

/// 叫它停了没有，隔多久看一眼旗。
const STOP_POLL: Duration = Duration::from_millis(100);

/// 提供者的工具替自己写的两句的模板。
#[derive(Clone)]
pub(super) struct Texts {
    /// 暂时不可用（`core/tool-results/unavailable.txt`），字段 `name`。
    pub(super) unavailable: String,
    /// 没在时限里答完（`core/tool-results/timed-out.txt`），字段 `name`、`seconds`。
    pub(super) timed_out: String,
}

/// 扩展传上来的 blob 在哪（施工 O-2 三补）：数据根（施工 SU-4 下起和主人共用一个存处）。
#[derive(Clone)]
pub(crate) struct Stores {
    pub(crate) root: DataRoot,
    /// 包声明的状态块（施工 TW-1）：工具结果带的 `state` 照它查。
    pub(crate) declared: Arc<crate::sessions::state::Declared>,
}

/// 提供者的一件工具。
pub(crate) struct RemoteTool {
    spec: Spec,
    venues: Venues,
    /// 登记时写的给人看的名字，照语言（施工 9-8 六补）。
    display_name: BTreeMap<String, String>,
    /// 登记时写的显示名后面跟哪一个参数（施工 S-6 上）。
    subject: Option<String>,
    timeout: Duration,
    package: String,
    /// 是哪个通讯平台专用的（施工 IM-3，[`Tool::platform`]）。
    platform: Option<String>,
    provided: Arc<Provided>,
    texts: Texts,
    stores: Stores,
}

/// 回应里的结果。
#[derive(Deserialize)]
struct Answer {
    blocks: Vec<Block>,
    #[serde(default)]
    error: bool,
    /// 要结束这一轮（施工 4-14，`protocol.md` 的 `tool.call`）：通讯平台的 `skip_reply` 这类。出错的不算。
    #[serde(default)]
    end_turn: bool,
    /// 她发出去的图（施工 X-19，`protocol.md` 的 `tool.call`）：至多 [`MOST_SENT`] 张，多了整份当写法不对。
    #[serde(default)]
    sent: Vec<Sent>,
    /// 给头看的结构化结果（施工 W-11 补，`providers.md`）：照 [`miyu_tool::detail`] 的通用形状读，读不成的丢掉、记一行。
    #[serde(default)]
    detail: Option<Value>,
    /// 给脚本的结构化结果（施工 X-33，`providers.md`）：任意 JSON，照收、交进 [`Done::data`]，出错的不交；不另截断。
    #[serde(default)]
    data: Option<Value>,
    /// 包在这个会话里的状态（施工 TW-1，设计 46）：照包声明的块查（`sessions/state.rs`），读不成的丢掉、记一行。`null` 当没写，
    /// 清掉写 `{}`。出错的不收。
    #[serde(default)]
    state: Option<Value>,
}

/// 一次至多发几张图。
const MOST_SENT: usize = 4;

impl RemoteTool {
    /// 包 `package` 提供的这一件。
    pub(super) fn new(
        checked: Checked,
        package: &str,
        provided: Arc<Provided>,
        texts: Texts,
        stores: Stores,
    ) -> RemoteTool {
        RemoteTool {
            spec: checked.spec,
            venues: checked.venues,
            display_name: checked.display_name,
            subject: checked.subject,
            timeout: checked.timeout,
            package: package.to_string(),
            platform: None,
            provided,
            texts,
            stores,
        }
    }

    /// 专用于平台 `platform`（施工 IM-3）：包接的平台里，名字以 `<平台>_` 开头的那一个；都不是的不分平台。
    pub(super) fn for_platform<'a>(
        mut self,
        platforms: impl IntoIterator<Item = &'a String>,
    ) -> RemoteTool {
        let name = self.spec.name.as_str();
        self.platform = platforms
            .into_iter()
            .find(|platform| {
                name.strip_prefix(platform.as_str())
                    .is_some_and(|rest| rest.starts_with('_'))
            })
            .cloned();
        self
    }

    /// 暂时不可用：说法同执行器替目录里没有的工具写的那一句。
    fn unavailable(&self) -> Done {
        let name = self.spec.name.as_str();
        Done {
            human: Some(Said::new("core/tool-results/unavailable").with("name", name)),
            ..failed(render(&self.texts.unavailable, &[("name", name)]))
        }
    }

    /// 没在时限里答完：秒数往上取整。
    fn timed_out(&self) -> Done {
        let name = self.spec.name.as_str();
        let seconds = self.timeout.as_millis().div_ceil(1000).to_string();
        Done {
            human: Some(
                Said::new("core/tool-results/timed-out")
                    .with("name", name)
                    .with("seconds", &seconds),
            ),
            ..failed(render(
                &self.texts.timed_out,
                &[("name", name), ("seconds", &seconds)],
            ))
        }
    }

    /// 照回应交回结果。
    fn answered(&self, outcome: Result<Result<Value, Value>, Gone>) -> Done {
        match outcome {
            Ok(Ok(result)) => match serde_json::from_value::<Answer>(result.clone()) {
                Ok(answer) if answer.sent.len() <= MOST_SENT => Done {
                    detail: answer
                        .detail
                        .filter(|_| !answer.error)
                        .and_then(|detail| self.shaped(&detail)),
                    data: answer.data.filter(|_| !answer.error),
                    effects: answer
                        .state
                        .filter(|_| !answer.error)
                        .and_then(|state| self.state(&state))
                        .into_iter()
                        .collect(),
                    error: answer.error,
                    blocks: answer.blocks,
                    end_turn: answer.end_turn && !answer.error,
                    // 本机路径不收：扩展说的位置不可信，头照 blob 取图（施工 X-19）。
                    sent: answer
                        .sent
                        .into_iter()
                        .map(|mut sent| {
                            sent.image.path = None;
                            sent
                        })
                        .collect(),
                    ..Done::ok("")
                },
                Ok(_) => failed(result.to_string()),
                Err(_) => failed(result.to_string()),
            },
            Ok(Err(error)) => failed(
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .map_or_else(|| error.to_string(), str::to_string),
            ),
            Err(Gone) => self.unavailable(),
        }
    }

    /// 照包声明的块读扩展交的 `state`：读不成的不要，记一行。
    fn state(&self, value: &Value) -> Option<miyu_tool::Effect> {
        match crate::sessions::state::read(&self.stores.declared, value) {
            Ok(state) => Some(miyu_tool::Effect::PackageState(
                miyu_kernel::event::PackageState {
                    package: self.package.clone(),
                    state,
                },
            )),
            Err(bad) => {
                tracing::warn!(target: "miyu::endpoint", package = self.package.as_str(), why = bad.why().as_str(), "package state dropped");
                None
            }
        }
    }

    /// 照通用形状读扩展交的 `detail`：读不成的不要，记一行（原因转述了工具的结果，不写进运行日志）。
    fn shaped(&self, detail: &Value) -> Option<Value> {
        match miyu_tool::detail::read(detail) {
            Ok(shaped) => Some(shaped.to_value()),
            Err(_) => {
                tracing::warn!(target: "miyu::endpoint", tool = self.spec.name.as_str(), "tool detail dropped");
                None
            }
        }
    }

    /// 交回的图、文件引用的 blob 要在附件存处里（施工 O-2 三补；施工 SU-4 下起扩展和主人共用一个存处，不用再拷）。
    /// 没有引用的、不知道是哪个会话的（`in_session` 是假的，测试里的假调用）原样交回；不在的记一行，照暂时不可用。
    async fn handed(&self, done: Done, in_session: bool) -> Done {
        let hashes = referenced(&done);
        if !in_session || hashes.is_empty() {
            return done;
        }
        let root = self.stores.root.clone();
        let there = tokio::task::spawn_blocking(move || {
            let blobs = miyu_store::blob::Blobs::new(root.blobs());
            hashes.iter().all(|hash| blobs.path(hash).is_file())
        })
        .await;
        match there {
            Ok(true) => done,
            Ok(false) | Err(_) => {
                tracing::warn!(target: "miyu::endpoint", tool = self.spec.name.as_str(), "tool blobs not found");
                self.unavailable()
            }
        }
    }
}

impl Tool for RemoteTool {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn venues(&self) -> Option<Venues> {
        Some(self.venues)
    }

    fn platform(&self) -> Option<&str> {
        self.platform.as_deref()
    }

    fn display_name(&self) -> BTreeMap<String, String> {
        self.display_name.clone()
    }

    fn subject(&self) -> Option<String> {
        self.subject.clone()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let ids = call.ids.as_ref();
            let Some(peer) = self.provided.peer(&self.package) else {
                return self.unavailable();
            };
            let args: Value = serde_json::from_str(&call.args).unwrap_or_else(|_| json!({}));
            let asked = ids.and_then(|ids| ids.asked.as_ref());
            let session = ids.map(|ids| ids.session.as_str());
            let in_session = ids.is_some();
            let call_id = ids.map(|ids| ids.call.to_string());
            let mut params = json!({
                "session": session,
                "call_id": call_id,
                "tool": self.spec.name,
                "args": args,
                "cwd": call.cwd,
                "owner": asked.is_some_and(|by| by.is_owner()),
            });
            if let Some(by) = asked {
                params["by"] = json!(by);
            }
            // 会话属主的 pictures（施工 Y-11 下）：画出来的图放这里，照数据根的真实位置接上；不建，扩展写之前自己建。会话外面的
            // 调用没有数据根，不写。只给本机的会话（施工 Y-11 下修）：场所会话关在自己的工作区里（施工 5-12），图存进主人家里
            // 她就读不到、发不出，几个群还共用一处；扩展照旧存工作区。不知道在哪个场所的也不给。
            let local = call
                .caller
                .as_ref()
                .is_some_and(|caller| caller.venue.as_str() == crate::list::LOCAL);
            if let (Some(root), true, true) = (call.data_root.as_deref(), ids.is_some(), local) {
                let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
                let pictures = miyu_store::root::user_dir(&root, "pictures");
                params["pictures"] = json!(pictures.display().to_string());
            }
            let mut cancel = Cancel {
                peer: peer.clone(),
                params: json!({"session": session, "call_id": call_id}),
                state: Sending::Armed,
            };
            let deadline = tokio::time::Instant::now() + self.timeout;
            let answer = peer.call("tool.call", params);
            tokio::pin!(answer);
            loop {
                tokio::select! {
                    outcome = &mut answer => {
                        cancel.state = Sending::Done;
                        return self.handed(self.answered(outcome), in_session).await;
                    }
                    // 超时：交回那一句，`tool.cancel` 由 `cancel` 丢掉时发。
                    () = tokio::time::sleep_until(deadline) => return self.timed_out(),
                    // 叫它停（打断改东西的调用）：告诉提供者，照样等它回，内核到点掐掉。
                    () = tokio::time::sleep(STOP_POLL), if cancel.state == Sending::Armed => {
                        if call.stop.stopped() {
                            cancel.send();
                        }
                    }
                }
            }
        })
    }
}

/// `tool.cancel` 发到哪一步了。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sending {
    /// 还没发，丢掉时发。
    Armed,
    /// 发过了。
    Sent,
    /// 回应到了，不用发。
    Done,
}

/// 发 `tool.cancel` 的：叫它停时发；回应没到就不等了的（超时、这次调用的 future 被丢掉）丢掉它时发。
struct Cancel {
    peer: Peer,
    params: Value,
    state: Sending,
}

impl Cancel {
    /// 还没发的发一次。
    fn send(&mut self) {
        if self.state == Sending::Armed {
            self.state = Sending::Sent;
            if !self.peer.notify("tool.cancel", self.params.clone()) {
                tracing::warn!(target: "miyu::endpoint", "tool.cancel not sent");
            }
        }
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        self.send();
    }
}

/// 照模板 `template` 换进字段 `fields`；模板坏了的交回空的。
/// 一份结果引用的 blob：图片、文件块，发出去的图，`images` 形状里的图（施工 W-11 补：读过一遍，这里读得成）。
pub(super) fn referenced(done: &Done) -> Vec<ContentHash> {
    let mut hashes = crate::attach::blobs_of(&done.blocks);
    hashes.extend(done.sent.iter().map(|sent| sent.image.blob.clone()));
    if let Some(Ok(detail)) = done.detail.as_ref().map(miyu_tool::detail::read) {
        hashes.extend(detail.blobs());
    }
    hashes
}

fn render(template: &str, fields: &[(&str, &str)]) -> String {
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    Template::parse(template)
        .ok()
        .and_then(|template| template.render(&fields).ok())
        .unwrap_or_default()
}

/// 出错，交回一段字。
fn failed(text: String) -> Done {
    Done {
        error: true,
        blocks: vec![Block::Text(Text { text })],
        ..Done::ok("")
    }
}
