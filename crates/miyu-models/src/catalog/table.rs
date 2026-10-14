//! 一家供应商的模型表（施工 V-5 中）：照名字排好的一排，找的时候二分。原来是 `BTreeMap`：八千多个模型，树的节点留着
//! 没用满的格子、另有内部节点，读进来约 6 MB，排成一排只要它的一半。表读好以后不再改。

use super::{CatalogModel, Text};

/// 一家供应商的模型：名字 → 资料，照名字的字节排。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelTable(Box<[(Text, CatalogModel)]>);

impl ModelTable {
    /// 照 `rows` 造：照名字排好，同名的只留后写的那一个（和原来往 `BTreeMap` 里一个个放的结果一样）。
    pub(crate) fn of(mut rows: Vec<(Text, CatalogModel)>) -> ModelTable {
        rows.reverse();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows.dedup_by(|later, kept| later.0 == kept.0);
        ModelTable(rows.into_boxed_slice())
    }

    /// 名字是 `name` 的那个模型。
    pub fn get(&self, name: &str) -> Option<&CatalogModel> {
        self.0
            .binary_search_by(|(key, _)| (**key).cmp(name))
            .ok()
            .map(|at| &self.0[at].1)
    }

    /// 有没有名字是 `name` 的模型。
    pub fn contains_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// 全部名字，照字节排。
    pub fn keys(&self) -> impl Iterator<Item = &Text> {
        self.0.iter().map(|(name, _)| name)
    }

    /// 全部资料，照名字排。
    pub fn values(&self) -> impl Iterator<Item = &CatalogModel> {
        self.0.iter().map(|(_, model)| model)
    }

    /// 名字和资料，照名字排。
    pub fn iter(&self) -> impl Iterator<Item = (&Text, &CatalogModel)> {
        self.0.iter().map(|(name, model)| (name, model))
    }

    /// 有几个模型。
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 一个模型都没有。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a> IntoIterator for &'a ModelTable {
    type Item = (&'a Text, &'a CatalogModel);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (Text, CatalogModel)>,
        fn(&'a (Text, CatalogModel)) -> (&'a Text, &'a CatalogModel),
    >;

    /// 名字和资料，照名字排（和原来 `BTreeMap` 的写法一样能 `for … in &表`）。
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(name, model)| (name, model))
    }
}
