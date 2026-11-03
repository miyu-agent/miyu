//! 一趟收录记到这时的（施工 G-16）：一个仓库里可以有几个包，每个包各自占编号、各自照编号拉黑。

use std::collections::BTreeMap;

use super::{Gathered, Rules};
use crate::index::Entry;

/// 这一趟收到这时的：编号表、记录、图标、条目。
pub(super) struct Tally<'a> {
    pub(super) names: &'a mut BTreeMap<String, String>,
    pub(super) notes: &'a mut Vec<String>,
    pub(super) logos: &'a mut BTreeMap<String, Vec<u8>>,
    pub(super) packages: &'a mut Vec<Entry>,
}

impl Tally<'_> {
    /// 仓库 `repo`（来源 `origin`）里收成的一个包：编号被别的仓库占了的、照编号拉黑的跳过，记一笔；没人占的这一趟占下。
    pub(super) fn take(
        &mut self,
        Gathered { entry, logo, note }: Gathered,
        repo: &str,
        origin: &str,
        rules: &Rules,
    ) {
        let Tally {
            names,
            notes,
            logos,
            packages,
        } = self;
        if let Some(note) = note {
            notes.push(format!("{repo}: {note}"));
        }
        match names.get(&entry.id) {
            Some(owner) if owner != origin => {
                notes.push(format!(
                    "{repo}: skipped, id {} belongs to {owner}",
                    entry.id
                ));
                return;
            }
            Some(_) => {}
            None => {
                names.insert(entry.id.clone(), origin.to_string());
                notes.push(format!("{repo}: claimed id {}", entry.id));
            }
        }
        // 拉黑的来源仓库在下载之前已经跳过，这里只剩照编号拉黑的。
        let blocked = rules
            .blocked
            .iter()
            .find(|blocked| blocked.id.as_deref() == Some(entry.id.as_str()));
        if let Some(blocked) = blocked {
            notes.push(format!("{repo}: blocked, {}", blocked.reason));
            return;
        }
        notes.push(format!("{repo}: {} {}", entry.id, entry.version));
        if let (Some(at), Some(bytes)) = (&entry.logo, logo) {
            logos.insert(at.path.clone(), bytes);
        }
        packages.push(entry);
    }
}
