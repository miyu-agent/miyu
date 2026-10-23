//! 她发出去的图（施工 X-19，`docs/blueprint/view.md`「条目」）：`tool.result.sent` 一张一条 `media`，排在那一步工具后面，和她的
//! 正文一样把前面那段时间线收起。

use miyu_kernel::event::{Event, ToolResult};

use super::Projector;
use crate::entry::{Attachment, Body, Entry, EntryId, Media};

impl Projector {
    /// 工具结果 `result` 里她发出去的图，各出一条。
    pub(super) fn sent(&mut self, event: &Event, result: &ToolResult) {
        if result.sent.is_empty() {
            return;
        }
        self.close_group();
        if let Some(running) = self.turn.as_mut() {
            running.spoke = true;
        }
        let turn = self.turn.as_ref().map(|t| t.id);
        for (n, sent) in result.sent.iter().enumerate() {
            let image = &sent.image;
            self.add(Entry {
                id: EntryId::sent(event.seq, n),
                body: Body::Media(Media {
                    image: Attachment {
                        kind: "image".to_string(),
                        name: image.name.as_ref().map(ToString::to_string),
                        media_type: image.media_type.as_str().to_string(),
                        blob: image.blob.clone(),
                        width: Some(image.width),
                        height: Some(image.height),
                        path: None,
                    },
                    alt: sent.alt.clone(),
                    sticker: sent.sticker,
                }),
                turn,
                hidden: false,
                at: event.at,
            });
        }
    }
}
