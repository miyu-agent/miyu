//! 一次改动（施工 V-42 的对照测试）：照这一种改一下对话。

use miyu_drivers::{EFFORT_OFF, EFFORT_ON, Inputs};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{ContentHash, ModelName};
use miyu_kernel::request::Message;

use super::talk::{DRIVERS, Talk};

/// 一次改动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    Turn,
    Again,
    Undo,
    Compact,
    Edit,
    Fact,
    Continue,
    System,
    Tools,
    Stable,
    Model,
    Limits,
    Inputs,
    Describe,
    Retext,
    Switch,
    Lose,
    Restore,
    Replace,
}

pub(super) const CHANGES: [Change; 19] = [
    Change::Turn,
    Change::Again,
    Change::Undo,
    Change::Compact,
    Change::Edit,
    Change::Fact,
    Change::Continue,
    Change::System,
    Change::Tools,
    Change::Stable,
    Change::Model,
    Change::Limits,
    Change::Inputs,
    Change::Describe,
    Change::Retext,
    Change::Switch,
    Change::Lose,
    Change::Restore,
    Change::Replace,
];

impl Talk {
    /// 照这一种改一下。
    pub(super) fn apply(&mut self, change: Change) {
        self.request.continuation = false;
        match change {
            Change::Turn => {
                if self.rng.chance(70) || self.request.messages.is_empty() {
                    self.user();
                }
                let calls = self.assistant();
                for call_id in calls {
                    if self.rng.chance(90) {
                        self.tool_result(call_id);
                    }
                }
            }
            Change::Again => {}
            Change::Undo => {
                let len = self.request.messages.len();
                let keep = len.saturating_sub(1 + self.rng.below(4));
                self.request.messages.truncate(keep);
            }
            Change::Compact => {
                let len = self.request.messages.len();
                let cut = self.rng.below(len + 1);
                let summary = Message::User {
                    blocks: vec![Block::Text(Text {
                        text: format!("checkpoint {}", self.words(2)),
                    })],
                };
                self.request.messages.splice(..cut, [summary]);
            }
            Change::Edit => {
                let len = self.request.messages.len();
                if len > 0 {
                    let at = self.rng.below(len);
                    let text = self.text();
                    match &mut self.request.messages[at] {
                        Message::User { blocks }
                        | Message::Assistant { blocks }
                        | Message::Tool { blocks, .. } => blocks.push(text),
                    }
                }
            }
            Change::Fact => {
                let text = self.text();
                match self.request.messages.last_mut() {
                    Some(Message::User { blocks }) => blocks.insert(0, text),
                    _ => self
                        .request
                        .messages
                        .push(Message::User { blocks: vec![text] }),
                }
            }
            Change::Continue => {
                if !matches!(
                    self.request.messages.last(),
                    Some(Message::Assistant { .. })
                ) {
                    self.assistant();
                }
                let text = self.text();
                self.request
                    .messages
                    .push(Message::User { blocks: vec![text] });
                self.request.continuation = true;
            }
            Change::System => {
                self.request.system = match self.rng.chance(20) {
                    true => String::new(),
                    false => self.words(3),
                };
            }
            Change::Tools => match self.rng.below(4) {
                0 => self.request.tools.clear(),
                1 => {
                    let tool = self.tool("placeholder");
                    self.request.tools.push(tool);
                }
                2 => {
                    self.request.tools.pop();
                }
                _ => {
                    if let Some(tool) = self.request.tools.first_mut() {
                        tool.description.push('!');
                    }
                }
            },
            Change::Stable => {
                self.request.stable = self.rng.below(self.request.messages.len() + 1);
            }
            Change::Model => {
                let name = format!("m-{}", self.rng.below(3));
                self.call.model = ModelName::parse(&name).expect("模型名合写法");
            }
            Change::Limits => {
                self.call.max_output = self
                    .rng
                    .chance(70)
                    .then(|| 1000 + self.rng.next() as u32 % 9000);
                self.call.temperature = self.rng.chance(40).then_some(0.5);
                self.call.effort = match self.rng.below(4) {
                    0 => None,
                    1 => Some(EFFORT_ON.to_string()),
                    2 => Some(EFFORT_OFF.to_string()),
                    _ => Some("high".to_string()),
                };
            }
            Change::Inputs => {
                self.call.inputs = Inputs {
                    images: self.rng.chance(60),
                    pdf: self.rng.chance(50),
                };
            }
            Change::Describe => {
                let images: Vec<ContentHash> = self
                    .request
                    .messages
                    .iter()
                    .flat_map(|message| match message {
                        Message::User { blocks }
                        | Message::Assistant { blocks }
                        | Message::Tool { blocks, .. } => blocks.iter(),
                    })
                    .filter_map(|block| match block {
                        Block::Image(image) => Some(image.blob.clone()),
                        _ => None,
                    })
                    .collect();
                if images.is_empty() || self.rng.chance(20) {
                    self.request.described.clear();
                } else {
                    let blob = self.rng.pick(&images).clone();
                    let words = self.words(3);
                    self.request.described.insert(blob, words);
                }
            }
            Change::Retext => self.retexted = !self.retexted,
            Change::Switch => self.driver = self.rng.below(DRIVERS),
            Change::Lose => {
                let known: Vec<ContentHash> = self.store.keys().cloned().collect();
                if !known.is_empty() {
                    let blob = self.rng.pick(&known).clone();
                    self.store.remove(&blob);
                }
            }
            Change::Restore => {
                self.store = self.known.clone();
            }
            Change::Replace => {
                // 同一个哈希下长短变了（存坏了）：执行器照实报的大小编。
                let known: Vec<ContentHash> = self.store.keys().cloned().collect();
                if !known.is_empty() {
                    let blob = self.rng.pick(&known).clone();
                    if let Some(bytes) = self.store.get_mut(&blob) {
                        bytes.push(b'!');
                    }
                }
            }
        }
        if self.request.stable > self.request.messages.len() {
            self.request.stable = self.request.messages.len();
        }
    }
}
