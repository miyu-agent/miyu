//! 她发出去的图（施工 X-19，`internal/blueprint/view.md`「条目」）：`tool.result.sent` 一张一条 `media`，排在那一步工具后面，和她的
//! 正文一样把前面那段时间线收起。语音（施工 SD-1）一段一条 `voice`，带上这一轮是谁开的（[`crate::entry::Voice`]）。

use miyu_kernel::event::{Event, Sent, ToolResult};

use super::Projector;
use crate::entry::{Attachment, Body, Entry, EntryId, Media, Voice};

impl Projector {
    /// 工具结果 `result` 里她发出去的图、语音，各出一条。
    pub(super) fn sent(&mut self, event: &Event, result: &ToolResult) {
        if result.sent.is_empty() {
            return;
        }
        self.close_group();
        if let Some(running) = self.turn.as_mut() {
            running.spoke = true;
        }
        let turn = self.turn.as_ref().map(|t| t.id);
        let asked = self
            .turn
            .as_ref()
            .map(|t| t.asked.clone())
            .unwrap_or_default();
        for (n, sent) in result.sent.iter().enumerate() {
            let body = match sent {
                Sent::Image(sent) => {
                    let image = &sent.image;
                    Body::Media(Media {
                        image: Attachment {
                            kind: "image".to_string(),
                            name: image.name.as_ref().map(ToString::to_string),
                            media_type: Some(image.media_type.as_str().to_string()),
                            blob: Some(image.blob.clone()),
                            width: Some(image.width),
                            height: Some(image.height),
                            path: None,
                            size: None,
                            path_only: false,
                        },
                        alt: sent.alt.clone(),
                        sticker: sent.sticker,
                    })
                }
                Sent::Voice(voice) => Body::Voice(Voice {
                    blob: voice.blob.clone(),
                    media_type: voice.media_type.as_str().to_string(),
                    name: voice.name.as_ref().map(ToString::to_string),
                    duration_ms: voice.duration_ms,
                    by: asked.0.clone(),
                    cause: asked.1.clone(),
                }),
            };
            self.add(Entry {
                id: EntryId::sent(event.seq, n),
                body,
                turn,
                hidden: false,
                at: event.at,
            });
        }
    }
}
