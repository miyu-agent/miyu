//! 整理好的目录（施工 V-5 中，`docs/blueprint/models.md`「怎么走」第二条第 1 条）：读好的目录照存法原样写一份，下次起来
//! 直接读它，不再从五 MB 多的原文现解。原文一个模型一个模型地扫三遍、每个字都要认一遍，约 22 ms；整理好的这份只有用得上
//! 的格，同样的字、价格、思考强度各写一次，别处写编号。
//!
//! 认不认这一份照 `of`：读的一方算的指纹（原文是哪一份、多大、什么时候改的，程序是哪一个），加上这里的 [`VERSION`]。对不上
//! 的、读不懂的、编号越界的都当没有，照原文现解。
//!
//! 两份索引（名字、规整过的名字）也照平铺的样子写下来（施工 V-8，版本 2）：读回来照编号拼，不再一个个规整、排序、去重。
//! 原来读回来约 8 ms，三成多花在现建索引上；核心起来时载入会话要等它读完。

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::index::Index;
use super::{
    Catalog, CatalogModel, CatalogProvider, Inputs, ModelTable, Price, Reasoning, Text, USD,
};

/// 写法的版本：这一份的样子变了就加一。
pub const VERSION: u32 = 2;

/// 一个模型一行：名字、显示名、家族、能不能调工具、输入、窗口、最大输出、价格、思考强度、能不能调温度、状态、发布日期、
/// 包名、交错思考的字段。字写 `texts` 里的编号，价格、思考强度写各自那一排里的编号。
type Row = (
    u32,
    Option<u32>,
    Option<u32>,
    Option<bool>,
    Option<Inputs>,
    Option<u64>,
    Option<u64>,
    Option<u32>,
    Option<u32>,
    Option<bool>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
);

/// 一家供应商：编号（`texts` 里的编号，和索引里的条目共用一份字）、显示名、环境变量、包名、地址、文档、模型。
type Provider<S> = (
    u32,
    Option<S>,
    Vec<S>,
    Option<S>,
    Option<S>,
    Option<S>,
    Vec<Row>,
);

/// 一份索引平铺的两排（施工 V-8，照 [`Index::parts`]）：键（字的编号）和它的条目从哪到哪；条目（供应商、模型的字的编号）。
type Flat = (Vec<(u32, u32, u32)>, Vec<(u32, u32)>);

/// 写的那一份。
#[derive(Serialize)]
struct Out<'a> {
    v: u32,
    of: &'a str,
    texts: Vec<&'a str>,
    prices: Vec<&'a Price>,
    reasonings: Vec<&'a Reasoning>,
    providers: Vec<Provider<&'a str>>,
    named: Flat,
    normalized: Flat,
}

/// 读的那一份。
#[derive(Deserialize)]
struct In<'a> {
    v: u32,
    #[serde(borrow)]
    of: Cow<'a, str>,
    #[serde(borrow)]
    texts: Vec<Cow<'a, str>>,
    prices: Vec<Price>,
    reasonings: Vec<Reasoning>,
    #[serde(borrow)]
    providers: Vec<Provider<Cow<'a, str>>>,
    named: Flat,
    normalized: Flat,
}

impl Catalog {
    /// 写成整理好的一份，指纹是 `of`。
    pub fn digest(&self, of: &str) -> String {
        let mut writer = Writer::default();
        let providers = self
            .providers
            .iter()
            .map(|(key, entry)| writer.provider(key, entry))
            .collect();
        let named = writer.index(&self.named);
        let normalized = writer.index(&self.normalized);
        let out = Out {
            v: VERSION,
            of,
            texts: writer.texts,
            prices: writer.prices,
            reasonings: writer.reasonings,
            providers,
            named,
            normalized,
        };
        serde_json::to_string(&out).unwrap_or_default()
    }

    /// 读整理好的一份：版本、指纹 `of` 对得上、读得懂、编号都不越界的才交回。
    pub fn from_digest(text: &str, of: &str) -> Option<Catalog> {
        let read: In<'_> = serde_json::from_str(text).ok()?;
        if read.v != VERSION || read.of != of {
            return None;
        }
        // 写的时候同样的字只写了一次：一个编号一份，供应商的编号、模型名、索引里的键和条目都照编号共用它。
        let texts: Vec<Text> = read.texts.iter().map(|text| Text::from(&**text)).collect();
        let text_at = |index: u32| texts.get(index as usize).cloned();
        let at = |index: Option<u32>| -> Option<Option<Text>> {
            index.map_or(Some(None), |index| text_at(index).map(Some))
        };
        let prices: Vec<Arc<Price>> = read.prices.into_iter().map(catalog_price).collect();
        let reasonings: Vec<Arc<Reasoning>> = read.reasonings.into_iter().map(Arc::new).collect();
        let mut catalog = Catalog::default();
        for (id, name, env, npm, api, doc, rows) in read.providers {
            let key = text_at(id)?;
            let mut models = Vec::with_capacity(rows.len());
            for row in rows {
                let model = CatalogModel {
                    name: at(row.1)?,
                    family: at(row.2)?,
                    tools: row.3,
                    inputs: row.4,
                    window: row.5,
                    max_output: row.6,
                    price: shared_at(&prices, row.7)?,
                    reasoning: shared_at(&reasonings, row.8)?,
                    temperature: row.9,
                    status: at(row.10)?,
                    release_date: at(row.11)?,
                    npm: at(row.12)?,
                    interleaved: at(row.13)?,
                };
                models.push((text_at(row.0)?, model));
            }
            let entry = CatalogProvider {
                id: key.to_string(),
                name: name.map(Cow::into_owned),
                env: env.into_iter().map(Cow::into_owned).collect(),
                npm: npm.map(Cow::into_owned),
                api: api.map(Cow::into_owned),
                doc: doc.map(Cow::into_owned),
                models: ModelTable::of(models),
            };
            catalog.providers.insert(key, entry);
        }
        catalog.named = index_of(&texts, read.named)?;
        catalog.normalized = index_of(&texts, read.normalized)?;
        Some(catalog)
    }
}

/// 照平铺的两排、字的编号拼回一份索引：编号越界的、拼不成（[`Index::from_parts`]）的没有。
fn index_of(texts: &[Text], (keys, entries): Flat) -> Option<Index> {
    let text = |index: u32| texts.get(index as usize).cloned();
    let keys = keys
        .into_iter()
        .map(|(key, from, to)| Some((text(key)?, from, to)))
        .collect::<Option<Vec<_>>>()?;
    let entries = entries
        .into_iter()
        .map(|(provider, model)| Some((text(provider)?, text(model)?)))
        .collect::<Option<Vec<_>>>()?;
    Index::from_parts(keys, entries)
}

/// 写的时候：同样的字、同一份价格、同一份思考强度只写一次，记下编号。
#[derive(Default)]
struct Writer<'a> {
    texts: Vec<&'a str>,
    text_at: BTreeMap<&'a str, u32>,
    prices: Vec<&'a Price>,
    price_at: BTreeMap<*const Price, u32>,
    reasonings: Vec<&'a Reasoning>,
    reasoning_at: BTreeMap<*const Reasoning, u32>,
}

impl<'a> Writer<'a> {
    fn provider(&mut self, key: &'a Text, entry: &'a CatalogProvider) -> Provider<&'a str> {
        let rows = entry
            .models
            .iter()
            .map(|(name, model)| self.row(name, model))
            .collect();
        (
            self.text(key),
            entry.name.as_deref(),
            entry.env.iter().map(String::as_str).collect(),
            entry.npm.as_deref(),
            entry.api.as_deref(),
            entry.doc.as_deref(),
            rows,
        )
    }

    fn row(&mut self, name: &'a Text, model: &'a CatalogModel) -> Row {
        (
            self.text(name),
            model.name.as_ref().map(|text| self.text(text)),
            model.family.as_ref().map(|text| self.text(text)),
            model.tools,
            model.inputs,
            model.window,
            model.max_output,
            model.price.as_ref().map(|price| self.price(price)),
            model
                .reasoning
                .as_ref()
                .map(|reasoning| self.reasoning(reasoning)),
            model.temperature,
            model.status.as_ref().map(|text| self.text(text)),
            model.release_date.as_ref().map(|text| self.text(text)),
            model.npm.as_ref().map(|text| self.text(text)),
            model.interleaved.as_ref().map(|text| self.text(text)),
        )
    }

    /// 一份索引照 [`Index::parts`] 写成两排，字写编号。
    fn index(&mut self, index: &'a Index) -> Flat {
        let (keys, entries) = index.parts();
        let keys = keys
            .iter()
            .map(|(key, from, to)| (self.text(key), *from, *to))
            .collect();
        let entries = entries
            .iter()
            .map(|(provider, model)| (self.text(provider), self.text(model)))
            .collect();
        (keys, entries)
    }

    fn text(&mut self, text: &'a Text) -> u32 {
        let next = to_u32(self.texts.len());
        *self.text_at.entry(&**text).or_insert_with(|| {
            self.texts.push(text);
            next
        })
    }

    fn price(&mut self, price: &'a Arc<Price>) -> u32 {
        let next = to_u32(self.prices.len());
        *self.price_at.entry(Arc::as_ptr(price)).or_insert_with(|| {
            self.prices.push(price);
            next
        })
    }

    fn reasoning(&mut self, reasoning: &'a Arc<Reasoning>) -> u32 {
        let next = to_u32(self.reasonings.len());
        *self
            .reasoning_at
            .entry(Arc::as_ptr(reasoning))
            .or_insert_with(|| {
                self.reasonings.push(reasoning);
                next
            })
    }
}

/// 读回来的价格：目录的币种照旧不另分配。
fn catalog_price(mut price: Price) -> Arc<Price> {
    if price.currency == USD {
        price.currency = Cow::Borrowed(USD);
    }
    Arc::new(price)
}

/// 那一排里编号是 `index` 的：外面一层是编号越没越界，里面一层是有没有。
fn shared_at<T>(shared: &[Arc<T>], index: Option<u32>) -> Option<Option<Arc<T>>> {
    index.map_or(Some(None), |index| {
        shared.get(index as usize).cloned().map(Some)
    })
}

/// 编号：目录里的字、价格不到四十亿个，放得进 `u32`。
fn to_u32(at: usize) -> u32 {
    u32::try_from(at).unwrap_or(u32::MAX)
}
