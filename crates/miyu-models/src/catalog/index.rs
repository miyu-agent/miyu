//! 照名字找条目的索引（施工 V-5 中）：键照字节排成一排，每个键记着它的条目在另一排里的哪一段。原来是「名字 → 条目的
//! `Vec`」的 `BTreeMap`：一个名字一次分配，八千多个模型两份索引一万多次分配、约 1.5 MB，平铺以后两排各一次分配。

use super::{Entry, Text};

/// 名字 → 条目。
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Index {
    /// 键，照字节排；后两格是它的条目在 `entries` 里从哪到哪。
    keys: Box<[(Text, u32, u32)]>,
    /// 条目：同一个键的挨着放，照字节排。
    entries: Box<[Entry]>,
}

impl Index {
    /// 照（键，条目）造：同一个键的条目照字节排，重复的只留一个。
    pub(super) fn of(mut pairs: Vec<(Text, Entry)>) -> Index {
        pairs.sort_unstable();
        pairs.dedup();
        let mut keys: Vec<(Text, u32, u32)> = Vec::new();
        let mut entries = Vec::with_capacity(pairs.len());
        for (key, entry) in pairs {
            let at = to_u32(entries.len());
            match keys.last_mut() {
                Some(last) if last.0 == key => last.2 = at + 1,
                _ => keys.push((key, at, at + 1)),
            }
            entries.push(entry);
        }
        Index {
            keys: keys.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        }
    }

    /// 键是 `key` 的条目，照字节排；没有的是空的。
    pub(super) fn get(&self, key: &str) -> &[Entry] {
        self.keys
            .binary_search_by(|(each, _, _)| (**each).cmp(key))
            .ok()
            .and_then(|at| {
                let (_, from, to) = &self.keys[at];
                self.entries.get(*from as usize..*to as usize)
            })
            .unwrap_or(&[])
    }
}

/// 条目的位置：目录里的模型不到四十亿个，放得进 `u32`。
fn to_u32(at: usize) -> u32 {
    u32::try_from(at).unwrap_or(u32::MAX)
}
