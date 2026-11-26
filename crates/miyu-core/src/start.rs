//! 起来的后半段，在运行时里（施工 Y-12 中上从 `main` 挪出来）：装停的信号（施工 CS-6），开套接字，建工作区、读清单和配置、换运行日志的级别、生成参考文件，接着 [`super::run`]。

use std::process::ExitCode;

use miyu_ipc::{Dirs, Lock};
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use super::{Failure, Live, Options, failed, load_packages, models, run, settings};

/// 后半段：见这一页开头。`logging` 是起来时照 `MIYU_LOG` 定的级别和装好的运行日志，读完配置照 `log.level` 换。
pub(super) async fn start(
    env: Env,
    (root, resources, lock): (DataRoot, ResourceRoot, Lock),
    options: Options,
    (level, log): (&miyu_log::Level, &miyu_log::Guard),
    catalog: models::Early,
) -> ExitCode {
    // 停的信号最先装（施工 CS-6）：起来这一段收到的也有计划地停，第二次当场强退。
    let stops = crate::stops::watch(log.flusher());
    let opened = match miyu_ipc::open_locked(&root, &Dirs::current(), lock) {
        Ok(opened) => opened,
        Err(error) => return failed("socket", Failure::socket(&error)),
    };
    if let Err(error) = root
        .prepare_workspace()
        .and_then(|()| root.prepare_private())
    {
        return failed("home", Failure::new("home_failed", error));
    }
    crate::leftovers::tidy(&root);
    let mut found = load_packages(&resources, &root);
    let places = miyu_store::packages::Packages::new(
        &resources,
        &root,
        crate::packages::distro_dirs(&resources),
    );
    let left = miyu_endpoint::packages::settings_left::left(&places, &root, &found);
    let packaged = settings::Packaged::of(&mut found);
    let config = settings::read(&root, env.home.as_deref(), &packaged, left);
    settings::log_level(&config, level, log);
    let locale = miyu_store::env::locale();
    // 生成参考文件在一条做完就退出的线程里（施工 LW-5，`heap.md`「怎么走」第 5 条）：读给人看的字、拼三份文件分配的写完就
    // 全放掉，放在主线程上会和长住的穿插着，主线程的 arena 碎成小片还不回去。等它写完再往下走，和原来一样在 `ready` 之前
    // 写好。线程起不来的照旧在这里写。
    let values = config.resolved().values();
    let generate = || {
        settings::generate(
            &root,
            &resources,
            locale.as_deref(),
            &values,
            config.items(),
            &packaged.manifests,
        );
    };
    std::thread::scope(|scope| {
        let thread = std::thread::Builder::new()
            .name("generate".to_string())
            .spawn_scoped(scope, generate);
        if let Err(error) = thread {
            tracing::warn!(target: crate::TARGET, error = %error, "generate thread not started");
            generate();
        }
    });
    // 照清单拼的配置项、清单的副本和配置的值只在读配置、生成参考文件时用（施工 LW-16）：下面的 `run` 一直等到核心停下才
    // 交回，这个函数的局部变量不在这里放下，就跟着留到那时（2026-11-22 量过：空闲时活着的堆里约 0.19 MB）。
    drop((packaged, values, places));
    let live = Live {
        levels: log.levels(),
        locale,
        found,
    };
    run(
        env,
        (root, resources, opened),
        (options, stops),
        (config, live),
        catalog,
    )
    .await
}
