//! 本机 embedding 模型的清单（施工 R-5 上写在 `miyu-embed` 里，R-5 中挪进来，`docs/blueprint/recall.md` 第四条第 2 款）：一份
//! TOML 写一个本机模型叫什么、几维、怎么取向量、最长几个词，和它的几个文件（SHA-256、多大）。它放在内置模型那个小程序包的
//! 目录里（`model.toml`，施工 R-5 三补）：换模型就是换包。
//!
//! 放在纯逻辑这一层，是因为两边要读同一份：小程序 `miyu-embed` 照它载入模型，核心照它核对包里的文件（R-5 三补起不再下载）；
//! 核心又不能依赖 `miyu-embed`（会把 ONNX Runtime 链进主程序）。这里只照原文读、查；读文件是用的一方的事。分词认 WordPiece 和
//! 字节级 BPE 两种（R-5 六补）；`cls` 以外的取法，换到那样的模型时再加：现在写别的，读的时候就拒，不悄悄算错。

use std::fmt;
use std::path::{Component, Path};

use serde::Deserialize;

/// 一份清单。读进来的都查过（[`Manifest::parse`]）：分词要的几种文件各正好一个，数在范围里，文件名是一个单纯的名字。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// 模型的名字，例如 `granite-embedding-97m-multilingual-r2`：一个单纯的名字。向量记的模型编号是 `local:<id>`（[`Manifest::model`]），设置页的
    /// 「内置模型」后面暗字写它。
    pub id: String,
    /// 向量几维。
    pub dims: usize,
    /// 怎么从模型的输出取一句的向量。
    pub pooling: Pooling,
    /// 一句最多几个词（连前后加的两个特殊的词）：超过的截掉后面的，留住后面那一个。
    pub max_tokens: usize,
    /// 怎么分词（不写的照 WordPiece，R-5 六补以前的清单照读）。
    #[serde(default)]
    pub tokenizer: Tokenizer,
    /// 模型的几个文件。
    pub files: Vec<ModelFile>,
}

/// 怎么取一句的向量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pooling {
    /// 取第一个词（`[CLS]`）那一格，再归一化（BERT 一族的做法）。
    Cls,
}

/// 怎么分词（R-5 六补）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Tokenizer {
    /// BERT 的 WordPiece：前后加 `[CLS]`、`[SEP]`（核心测试用的手算小模型照它）。
    #[default]
    Wordpiece,
    /// 字节级 BPE（granite r2 这一类）：要 `merges` 那个文件。
    Bpe {
        /// 前面加的特殊的词，例如 `<|startoftext|>`。
        start: String,
        /// 后面加的，例如 `<|return|>`。
        end: String,
    },
}

/// 一个文件是做什么的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// ONNX 模型。
    Model,
    /// 词表：一行一个词，行号就是编号。
    Vocab,
    /// BPE 的合并：一行一条「左 右」，行号就是先后（R-5 六补）。
    Merges,
}

impl Role {
    /// 清单里的写法。
    fn as_str(self) -> &'static str {
        match self {
            Role::Model => "model",
            Role::Vocab => "vocab",
            Role::Merges => "merges",
        }
    }
}

/// 模型的一个文件。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    /// 做什么的。
    pub role: Role,
    /// 放在包目录里叫什么：一个单纯的名字，不带目录。
    pub name: String,
    /// 文件内容的 SHA-256，十六进制（核心第一次用时照它核对包里的文件）。
    pub sha256: String,
    /// 多少字节（核心核对时先比它）。
    pub size: u64,
}

/// 清单不合写法：TOML 坏了、少格、多格，或者查下来不对。原话是英文短句（小程序照它回给核心，核心记进日志）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError(pub String);

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid manifest: {}", self.0)
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    /// 照 TOML 的原文 `text` 读一份，再查一遍。
    ///
    /// # Errors
    ///
    /// TOML 坏了、少格或多格、取法或分词不认识；`id`、文件名带目录；`model`、`vocab`（BPE 的另有 `merges`）不是各正好一个，
    /// WordPiece 的有 `merges`；`dims` 是 0；`max_tokens` 放不下前后两个特殊的词。
    pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
        let manifest: Manifest = toml_edit::de::from_str(text)
            .map_err(|error| ManifestError(error.to_string().trim_end().to_string()))?;
        manifest.check().map_err(ManifestError)?;
        Ok(manifest)
    }

    /// 查读进来的几格，交回第一处不对的。
    fn check(&self) -> Result<(), String> {
        // 编号进向量的模型编号、给人看的暗字：一个单纯的名字，带目录的不收。
        if !plain(&self.id) {
            return Err(format!("id {} is not a plain name", self.id));
        }
        if self.dims == 0 {
            return Err("dims must be at least 1".to_string());
        }
        if self.max_tokens < 2 {
            return Err("max_tokens must be at least 2".to_string());
        }
        let merges = matches!(self.tokenizer, Tokenizer::Bpe { .. });
        for (role, wanted) in [
            (Role::Model, true),
            (Role::Vocab, true),
            (Role::Merges, merges),
        ] {
            match (
                self.files.iter().filter(|file| file.role == role).count(),
                wanted,
            ) {
                (0, true) => return Err(format!("no file is {}", role.as_str())),
                (1, true) | (0, false) => {}
                (_, true) => return Err(format!("two files are {}", role.as_str())),
                (_, false) => {
                    return Err(format!(
                        "a wordpiece tokenizer has no {} file",
                        role.as_str()
                    ));
                }
            }
        }
        match self.files.iter().find(|file| !plain(&file.name)) {
            Some(file) => Err(format!("file name {} is not a plain name", file.name)),
            None => Ok(()),
        }
    }

    /// 向量记的模型编号：`local:<id>`（`recall.md` 第三条第 1 款）。
    pub fn model(&self) -> String {
        format!("local:{}", self.id)
    }

    /// 做 `role` 的那个文件。
    ///
    /// # Panics
    ///
    /// 照 [`Manifest::parse`] 读进来的不会：每种正好一个。自己拼的清单少了那一种才会。
    pub fn file(&self, role: Role) -> &ModelFile {
        self.files
            .iter()
            .find(|file| file.role == role)
            .unwrap_or_else(|| panic!("清单里没有 {} 的文件", role.as_str()))
    }
}

/// `name` 是不是一个单纯的文件名：不空、不是 `.`、`..`，两种平台的目录分隔符都不带（清单在三个平台上一样用）。
fn plain(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    ) && !name.contains(['/', '\\'])
}

#[cfg(test)]
mod tests;
