//! 列会话时索引的行怎么读进来（施工 YC-13，`docs/blueprint/store/index.md`「怎么走」第 3 条第 1 款）。不带上限的一次读出
//! 全部；带上限的照放会话的目录的先后一批批读，只读走得到的那一截：会话有几千个、只要头几十个的，不把整张表解出来。
//!
//! 一批是目录里挨着的一截会话，照它两头的编号问索引（[`SessionIndex::rows_between`]）。第一批照上限（至少 [`FIRST`] 个），
//! 后面一批比一批大一倍：挑得严的（几千个里只要一个场所的）走完全部也只问十几次。

use std::collections::BTreeMap;

use miyu_kernel::id::SessionId;
use miyu_store::index::{IndexError, Row, SessionIndex};

/// 带上限时第一批至少读几行：上限很小的（找回场所会话只要一个）也不一个个问。
const FIRST: usize = 32;

/// 照目录的先后交出索引的行。
pub(super) struct Pages<'a> {
    index: Option<&'a SessionIndex>,
    /// 放会话的目录里的会话，从新到旧。
    ids: &'a [SessionId],
    /// 读进来还没交出去的行。
    rows: BTreeMap<SessionId, Row>,
    /// `ids` 的前这么多个读过了。
    read: usize,
    /// 下一批读几个。
    batch: usize,
}

impl<'a> Pages<'a> {
    /// `limit` 是空的一次读出全部（和没有上限时一样，目录是空的也读，坏了的当场认出来）；不是空的等走到了再读。
    pub(super) fn new(
        index: Option<&'a SessionIndex>,
        ids: &'a [SessionId],
        limit: Option<usize>,
    ) -> Pages<'a> {
        let mut pages = Pages {
            index,
            ids,
            rows: BTreeMap::new(),
            read: 0,
            batch: limit.map_or(0, |limit| limit.max(FIRST)),
        };
        if limit.is_none() {
            pages.read = ids.len();
            pages.load(SessionIndex::rows);
        }
        pages
    }

    /// 目录里第 `at` 个会话的那一行：没有的、索引读不了的是空的。`at` 只往后走。
    pub(super) fn take(&mut self, at: usize) -> Option<Row> {
        if at >= self.read {
            let end = at.saturating_add(self.batch).min(self.ids.len());
            let (newest, oldest) = (self.ids.get(at)?, self.ids.get(end.checked_sub(1)?)?);
            self.read = end;
            self.batch = self.batch.saturating_mul(2);
            self.load(|index| index.rows_between(oldest, newest));
        }
        self.rows.remove(self.ids.get(at)?)
    }

    /// 读一批。读出坏了的（有一行读不懂、SQLite 说坏了）：删掉重建，这以后当一行都没有，每个会话整份读，读完的照样写进
    /// 新的里。
    fn load(
        &mut self,
        read: impl FnOnce(&SessionIndex) -> Result<BTreeMap<SessionId, Row>, IndexError>,
    ) {
        let Some(index) = self.index else {
            return;
        };
        match read(index) {
            Ok(rows) => {
                #[cfg(test)]
                super::tests::loaded(rows.len());
                self.rows = rows;
            }
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", error = %error, "session index not read");
                if let Err(error) = index.reset() {
                    tracing::warn!(target: "miyu::endpoint", error = %error, "session index unusable");
                }
                self.rows = BTreeMap::new();
                self.index = None;
                self.read = self.ids.len();
            }
        }
    }
}
