//! 别的技能目录（施工 PT-2 外）：家目录在工作目录下的 `me/`，核心的配置清单里有读别的技能目录那两项。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use super::{Home, TOKEN};

impl Home {
    /// 家目录：工作目录下的 `me/`（施工 PT-2 外：别的技能目录的 `~` 照它拼）。别的测试没有家目录，碰不到本机真的
    /// `~/.agents/skills/`。
    pub fn me(&self) -> PathBuf {
        self.work.join("me")
    }

    /// 在家目录 [`Home::me`] 下的 `relative` 写一个技能 `name`（施工 PT-2 外）。
    pub fn write_elsewhere(&self, relative: &str, name: &str, description: &str) {
        let file = self.me().join(relative).join(name).join("SKILL.md");
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
        let text = format!("---\nname: {name}\ndescription: {description}\n---\n\n正文\n");
        std::fs::write(&file, text).expect("写得进");
    }

    /// 一份核心，家目录是 [`Home::me`]，配置清单里有读别的技能目录那两项（施工 PT-2 外），照磁盘上的 `config.toml` 读；工具目录
    /// 是 `tools`，沙盒当能用。
    pub fn core_elsewhere(&self, script: &Script, tools: Catalog) -> Arc<Core> {
        let home = self.me();
        let config = miyu_endpoint::config::Config::load(
            &self.root,
            Some(&home),
            miyu_store::skills::external::ITEMS.to_vec(),
            miyu_endpoint::config::Environment::of(&[]),
        );
        Arc::new(
            self.core_full(script, tools, Some(home), TOKEN)
                .with_config(config)
                .with_sandbox(miyu_sandbox::Availability::Usable(PathBuf::from(
                    "miyu-sandbox",
                ))),
        )
    }
}
