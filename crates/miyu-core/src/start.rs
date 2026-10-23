//! 起来的后半段，在运行时里（施工 Y-12 中上从 `main` 挪出来）：开套接字，找机主（[`crate::oobe::owner`]），还没有的先跑设置那一段
//! （[`crate::oobe::setup`]）；有了机主，照他建家目录、读两层清单和配置、换运行日志的级别、生成参考文件，接着 [`super::run`]。
//! 设置那一段和完整的那一份用同一个监听器、同一把锁。

use std::process::ExitCode;

use miyu_ipc::{Dirs, Lock, Ready};
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use super::{Live, Options, failed, load_packages, models, oobe, run, say, serve, settings};

/// 后半段：见这一页开头。`logging` 是起来时照 `MIYU_LOG` 定的级别和装好的运行日志，读完配置照 `log.level` 换。
pub(super) async fn start(
    env: Env,
    (root, resources, lock): (DataRoot, ResourceRoot, Lock),
    options: Options,
    (level, log): (&miyu_log::Level, &miyu_log::Guard),
    catalog: models::Early,
) -> ExitCode {
    let mut opened = match miyu_ipc::open_locked(&root, &Dirs::current(), lock) {
        Ok(opened) => opened,
        Err(error) => return failed("socket", error.to_string()),
    };
    let (owner, said) = match oobe::owner(&root) {
        Some(owner) => (owner, false),
        None => {
            // 拉起它的头在等这一行：设置那一段就答，头连进来走 OOBE。
            say(&Ready::Ready);
            let set = oobe::setup(
                (&env, &root, &resources),
                (&mut opened.listener, &opened.token),
                (serve::signal(), options.idle),
            )
            .await;
            match set {
                Ok(Some(owner)) => (owner, true),
                Ok(None) => return ExitCode::SUCCESS,
                Err(reason) => return failed("setup", reason),
            }
        }
    };
    if let Err(error) = root.prepare_home(&owner) {
        return failed("home", error.to_string());
    }
    let mut found = load_packages(&resources, &root, &owner);
    let packaged = settings::Packaged::of(&mut found);
    let config = settings::read(&root, &owner, env.home.as_deref(), &packaged);
    settings::log_level(&config, level, log);
    let locale = miyu_store::env::locale();
    settings::generate(
        &root,
        &resources,
        locale.as_deref(),
        &config.resolved().values(),
        config.items(),
        &packaged.manifests,
    );
    let live = Live {
        levels: log.levels(),
        locale,
        found,
    };
    run(
        env,
        (root, resources, opened),
        options,
        (config, live),
        (catalog, owner, said),
    )
    .await
}
