//! 照名字找条目的索引（施工 V-5 中）：键照字节排成一排，每个键记着它的条目在另一排里的哪一段。原来是「名字 → 条目的
//! `Vec`」的 `BTreeMap`：一个名字一次分配，八千多个模型两份索引一万多次分配、约 1.5 MB，平铺以后两排各一次分配。
//! 整理好的目录把这两排原样写下来（施工 V-8），读回来照 [`Index::from_parts`] 拼，不再现规整、现排序。

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

    /// 平铺的两排（施工 V-8：整理好的目录照这个样子写下来）：键和它的条目从哪到哪，条目。
    pub(super) fn parts(&self) -> (&[(Text, u32, u32)], &[Entry]) {
        (&self.keys, &self.entries)
    }

    /// 照 [`Index::parts`] 的两排拼回来，不再排序（施工 V-8）。要和 [`Index::of`] 造出来的一样：键严格照字节升序，每个键
    /// 的那一段不空、首尾相接地盖住全部条目，段里照字节升序、不重复；不是这样的交回空的（整理好的那一份坏了，照原文重解）。
    pub(super) fn from_parts(keys: Vec<(Text, u32, u32)>, entries: Vec<Entry>) -> Option<Index> {
        let mut next = 0;
        for (at, (key, from, to)) in keys.iter().enumerate() {
            if *from != next || to <= from || at > 0 && keys[at - 1].0 >= *key {
                return None;
            }
            let range = entries.get(*from as usize..*to as usize)?;
            if range.windows(2).any(|pair| pair[0] >= pair[1]) {
                return None;
            }
            next = *to;
        }
        (next as usize == entries.len()).then(|| Index {
            keys: keys.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        })
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
