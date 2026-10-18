//! 读清单的文件（`recall.md` 第四条第 2 款）：内置语义模型的包里那一份读得出、每一格对、没有下载地址（R-5 三补；R-5 七补换成
//! granite）；文件读不了
//! 说是哪个。原文怎么读、怎么查在
//! `miyu_recall::embedding` 的单元测试里。

use miyu_embed::manifest::{self, Pooling, Role, Tokenizer};

use crate::support::shipped_manifest;

#[test]
fn the_shipped_one_reads() {
    let manifest = manifest::read(&shipped_manifest()).expect("出厂的读得出");
    assert_eq!(manifest.id, "granite-embedding-97m-multilingual-r2");
    assert_eq!(
        manifest.model(),
        "local:granite-embedding-97m-multilingual-r2"
    );
    assert_eq!(manifest.dims, 384);
    assert_eq!(manifest.pooling, Pooling::Cls);
    assert_eq!(manifest.max_tokens, 512);
    assert_eq!(
        manifest.tokenizer,
        Tokenizer::Bpe {
            start: "<|startoftext|>".to_string(),
            end: "<|return|>".to_string()
        }
    );
    let model = manifest.file(Role::Model);
    assert_eq!(model.name, "model_q4.onnx");
    assert_eq!(model.size, 61_181_663);
    assert_eq!(
        model.sha256, "66dfce3ab5462573ad2b7e578a1ef676eecad0e784c2373a01dad1cf62041f7a",
        "和 Release 的 SHA256SUMS 一样"
    );
    assert_eq!(manifest.file(Role::Vocab).name, "vocab.txt");
    assert_eq!(manifest.file(Role::Merges).name, "merges.txt");
}

#[test]
fn a_missing_file_says_where() {
    let path = std::path::Path::new("/nonexistent/manifest.toml");
    let error = manifest::read(path).expect_err("没有这个文件");
    assert!(
        error.starts_with("cannot read /nonexistent/manifest.toml"),
        "{error}"
    );
}
