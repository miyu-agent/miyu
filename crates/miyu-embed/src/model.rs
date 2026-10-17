//! 照清单载入一份模型、算一句的向量（施工 R-5 上，`docs/blueprint/recall.md` 第四条第 4 款）：ONNX Runtime 静态链接在
//! 这个小程序里，主程序不带。一次一条、单线程：旧版实测一次 32 条、16 线程时内存冲到 2 GB 不还。

use std::fmt;
use std::path::{Path, PathBuf};

use ort::session::Session;
use ort::value::Tensor;

use crate::manifest::{self, Manifest, Pooling, Role};
use crate::{bpe, wordpiece};

/// 一份载入了的模型。
#[derive(Debug)]
pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    /// 模型声明了的几个输入，照它声明的先后（R-5 六补：granite 没有 `token_type_ids`）。
    inputs: Vec<Input>,
    manifest: Manifest,
}

/// 照清单的分词（R-5 六补）。
#[derive(Debug)]
enum Tokenizer {
    Wordpiece(wordpiece::Tokenizer),
    /// 装箱：比 WordPiece 那一份大得多（正则、两张表）。
    Bpe(Box<bpe::Tokenizer>),
}

impl Tokenizer {
    fn encode(&self, text: &str, limit: usize) -> Vec<i64> {
        match self {
            Tokenizer::Wordpiece(tokenizer) => tokenizer.encode(text, limit),
            Tokenizer::Bpe(tokenizer) => tokenizer.encode(text, limit),
        }
    }
}

/// 模型的一个输入：认这三种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
    /// `input_ids`：词的编号。
    Ids,
    /// `attention_mask`：全是 1。
    Mask,
    /// `token_type_ids`：全是 0。
    Types,
}

impl Input {
    fn name(self) -> &'static str {
        match self {
            Input::Ids => "input_ids",
            Input::Mask => "attention_mask",
            Input::Types => "token_type_ids",
        }
    }
}

/// 载入不了。
#[derive(Debug)]
pub enum LoadError {
    /// 清单写的文件在目录里没有。
    Missing(PathBuf),
    /// 词表（BPE 的另有合并）读不了、不合写法。
    Vocab {
        /// 哪个文件。
        path: PathBuf,
        /// 为什么。
        error: String,
    },
    /// ONNX Runtime 载入不了模型。
    Model {
        /// 哪个文件。
        path: PathBuf,
        /// ONNX Runtime 说的原因。
        error: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Missing(path) => write!(f, "cannot find {}", path.display()),
            LoadError::Vocab { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            LoadError::Model { path, error } => {
                write!(f, "cannot load {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// 一句算不出来。
#[derive(Debug, Clone, PartialEq)]
pub enum EmbedError {
    /// ONNX Runtime 跑不了：输入的名字、类型对不上，或者跑的时候出错。
    Run(String),
    /// 模型交回来的最后一维和清单的 `dims` 对不上：清单写错了。
    Dims {
        /// 模型交回来的。
        got: i64,
        /// 清单写的。
        want: usize,
    },
    /// 取出来的那一格全是零、或者有不是数的，归一化不了。
    Degenerate,
}

impl fmt::Display for EmbedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbedError::Run(error) => write!(f, "cannot run the model: {error}"),
            EmbedError::Dims { got, want } => {
                write!(
                    f,
                    "the model gives {got} dimensions, the manifest says {want}"
                )
            }
            EmbedError::Degenerate => write!(f, "the vector is zero or not a number"),
        }
    }
}

impl std::error::Error for EmbedError {}

impl Embedder {
    /// 照清单 `manifest` 载入目录 `dir` 里的模型和词表。
    ///
    /// # Errors
    ///
    /// 文件没有，词表（BPE 的另有合并）读不了、不合写法，ONNX Runtime 载入不了模型，模型要的输入不认识。
    pub fn load(manifest: Manifest, dir: &Path) -> Result<Embedder, LoadError> {
        let model = dir.join(&manifest.file(Role::Model).name);
        let vocab = dir.join(&manifest.file(Role::Vocab).name);
        let merges = match manifest.tokenizer {
            manifest::Tokenizer::Wordpiece => None,
            manifest::Tokenizer::Bpe { .. } => Some(dir.join(&manifest.file(Role::Merges).name)),
        };
        for path in [Some(&model), Some(&vocab), merges.as_ref()]
            .into_iter()
            .flatten()
        {
            if !path.is_file() {
                return Err(LoadError::Missing(path.clone()));
            }
        }
        // 切词表（granite 那份六十来万条，约 150 毫秒）和 ONNX Runtime 建会话（约 170 毫秒）各不相干，一起做；算向量照旧单线程。
        let (tokenizer, session) = std::thread::scope(|scope| {
            let tokenizer = scope.spawn(|| tokenizer(&manifest, &vocab, merges.as_deref()));
            let session = session(&model);
            let tokenizer = tokenizer
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            (tokenizer, session)
        });
        let (tokenizer, (session, inputs)) = (tokenizer?, session?);
        Ok(Embedder {
            session,
            tokenizer,
            inputs,
            manifest,
        })
    }

    /// 向量记的模型编号（`local:<id>`）。
    pub fn model(&self) -> String {
        self.manifest.model()
    }

    /// 向量几维。
    pub fn dims(&self) -> usize {
        self.manifest.dims
    }

    /// 算 `text` 的向量：照清单切词、截断，模型要哪几个输入喂哪几个，跑一次，取第一个词那一格，归一化。
    ///
    /// # Errors
    ///
    /// ONNX Runtime 跑不了；模型交回来的维数和清单对不上；取出来的全是零、或者有不是数的。
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let ids = self.tokenizer.encode(text, self.manifest.max_tokens);
        let shape = [1, ids.len()];
        let run = |error: ort::Error| EmbedError::Run(error.to_string());
        let count = ids.len();
        let mut ids = Some(ids);
        let mut values = Vec::with_capacity(self.inputs.len());
        for input in &self.inputs {
            let value = match input {
                Input::Ids => ids.take().unwrap_or_default(),
                Input::Mask => vec![1_i64; count],
                Input::Types => vec![0_i64; count],
            };
            values.push((
                input.name(),
                Tensor::from_array((shape, value)).map_err(run)?,
            ));
        }
        let outputs = self.session.run(values).map_err(run)?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(run)?;
        let want = self.manifest.dims;
        let got = shape.last().copied().unwrap_or(0);
        if usize::try_from(got).ok() != Some(want) {
            return Err(EmbedError::Dims { got, want });
        }
        let vector = match self.manifest.pooling {
            Pooling::Cls => values.get(..want),
        };
        normalized(vector.ok_or(EmbedError::Dims { got, want })?)
    }
}

/// 照清单的分词读词表（BPE 的另读合并）。
fn tokenizer(
    manifest: &Manifest,
    vocab: &Path,
    merges: Option<&Path>,
) -> Result<Tokenizer, LoadError> {
    let bad = |path: &Path, error: String| LoadError::Vocab {
        path: path.to_path_buf(),
        error,
    };
    let text = std::fs::read_to_string(vocab).map_err(|error| bad(vocab, error.to_string()))?;
    match (&manifest.tokenizer, merges) {
        (manifest::Tokenizer::Bpe { start, end }, Some(merges)) => {
            let pairs =
                std::fs::read_to_string(merges).map_err(|error| bad(merges, error.to_string()))?;
            bpe::Tokenizer::new(&text, &pairs, start, end)
                .map(|tokenizer| Tokenizer::Bpe(Box::new(tokenizer)))
                .map_err(|error| bad(vocab, error.to_string()))
        }
        _ => wordpiece::Tokenizer::new(&text)
            .map(Tokenizer::Wordpiece)
            .map_err(|error| bad(vocab, error.to_string())),
    }
}

/// ONNX Runtime 载入模型（一次一条、单线程），连同它声明的几个输入：只认三种，要别的、没有 `input_ids` 的载入不了。
fn session(model: &Path) -> Result<(Session, Vec<Input>), LoadError> {
    // 每一步的错各是一种类型（带着造到一半的那一个），都照原话交上去。
    let failed = |error: &dyn fmt::Display| LoadError::Model {
        path: model.to_path_buf(),
        error: error.to_string(),
    };
    let session = Session::builder()
        .map_err(|error| failed(&error))?
        .with_intra_threads(1)
        .map_err(|error| failed(&error))?
        .with_inter_threads(1)
        .map_err(|error| failed(&error))?
        .commit_from_file(model)
        .map_err(|error| failed(&error))?;
    let mut inputs = Vec::new();
    for outlet in session.inputs() {
        let input = [Input::Ids, Input::Mask, Input::Types]
            .into_iter()
            .find(|input| input.name() == outlet.name())
            .ok_or_else(|| failed(&format!("the model wants an input named {}", outlet.name())))?;
        inputs.push(input);
    }
    if !inputs.contains(&Input::Ids) {
        return Err(failed(&"the model has no input named input_ids"));
    }
    Ok((session, inputs))
}

/// `vector` 除以它的长度：以后照点积算相似度（`recall.md` 第三条第 2 款）。
fn normalized(vector: &[f32]) -> Result<Vec<f32>, EmbedError> {
    let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if length == 0.0 || !length.is_finite() {
        return Err(EmbedError::Degenerate);
    }
    Ok(vector.iter().map(|x| x / length).collect())
}
