//! 不经核心读出对话模型（施工 RC-1）：照磁盘上的系统配置、密钥文件、资源目录的档案和目录算，读不了的说是哪一份，什么都不写。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_models::provider::Driver;
use miyu_store::env::Platform;

use super::*;

/// 一个临时的数据根（还没建）和它的环境：资源目录是源码树的，缓存目录在它里面。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-core-rescue-{}-{n}", std::process::id())))
    }

    fn env(&self) -> Env {
        let cache = self.0.join("cache").into_os_string();
        Env {
            platform: Platform::current(),
            miyu_home: Some(self.0.clone().into_os_string()),
            home: None,
            xdg_cache_home: Some(cache.clone()),
            local_app_data: Some(cache),
            miyu_resources: Some(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../resources")
                    .into_os_string(),
            ),
            exe: None,
        }
    }

    /// 写数据根里的 `file`。
    fn put(&self, file: PathBuf, text: &str) {
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
        std::fs::write(file, text).expect("写得进");
    }

    fn root(&self) -> DataRoot {
        DataRoot::at(self.0.clone())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

#[test]
fn a_known_provider_is_read_from_the_profile_and_the_secrets_file() {
    let scratch = Scratch::new();
    scratch.put(
        scratch.root().config_file(),
        "[providers.deepseek]\nkey = { secret = \"deepseek\" }\n\n[models]\nchat = \"deepseek/deepseek-v4-flash\"\n",
    );
    scratch.put(scratch.root().secrets_file(), "deepseek = \"sk-test\"\n");
    let direct = model_in(&scratch.env()).expect("用得了");
    assert_eq!(direct.base_url, "https://api.deepseek.com");
    assert_eq!(direct.model, "deepseek-v4-flash");
    assert_eq!(direct.driver, Driver::OpenAiChat);
    assert_eq!(direct.key.as_ref().map(Secret::expose), Some("sk-test"));
}

#[test]
fn the_address_comes_from_the_catalog_when_nothing_else_has_it() {
    // OpenRouter 的档案只写了驱动，地址照资源目录里 models.dev 的快照。
    let scratch = Scratch::new();
    scratch.put(
        scratch.root().config_file(),
        "[providers.openrouter]\nkey = { secret = \"or\" }\n\n[models]\nchat = \"openrouter/a/b\"\n",
    );
    scratch.put(scratch.root().secrets_file(), "or = \"sk-or\"\n");
    let direct = model_in(&scratch.env()).expect("用得了");
    assert_eq!(direct.base_url, "https://openrouter.ai/api/v1");
    assert_eq!(direct.model, "a/b");
    assert_eq!(direct.driver, Driver::OpenAiChat);
}

#[test]
fn a_broken_file_is_named_and_its_text_is_not_repeated() {
    let scratch = Scratch::new();
    scratch.put(scratch.root().config_file(), "[models\nchat = 1\n");
    let said = model_in(&scratch.env()).expect_err("配置写坏了");
    assert!(said.contains("config.toml: line 1: "), "{said}");

    scratch.put(
        scratch.root().config_file(),
        "[providers.deepseek]\nkey = { secret = \"deepseek\" }\n\n[models]\nchat = \"deepseek/m\"\n",
    );
    scratch.put(scratch.root().secrets_file(), "deepseek = \"sk-leak\n");
    let said = model_in(&scratch.env()).expect_err("密钥文件写坏了");
    assert!(said.contains("secrets.toml: line 1: "), "{said}");
    assert!(!said.contains("sk-leak"), "不带原文：{said}");
}

#[test]
fn nothing_is_written_even_when_the_data_root_is_missing() {
    let scratch = Scratch::new();
    assert_eq!(
        model_in(&scratch.env()).expect_err("没配"),
        miyu_models::provider::NOT_CONFIGURED
    );
    assert!(!scratch.0.exists(), "数据根没建");
}
