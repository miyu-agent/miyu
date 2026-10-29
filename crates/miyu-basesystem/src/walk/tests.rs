//! 叫停：丢掉这次调用的 future，阻塞线程里走目录的活就停下。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use miyu_tool::Stop;

use std::borrow::Cow;
use std::sync::Arc;

use miyu_config::secret::{Reference, Secret};
use miyu_config::{Value, Values};

use super::{Fence, files};
use crate::blocking::blocking;

/// 一个用完就删的临时目录，里面有几个文件。
struct Temp(std::path::PathBuf);

impl Drop for Temp {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 什么都不挡的篱笆。
fn open() -> Fence {
    Fence {
        data_root: None,
        workspace: None,
        private: Vec::new(),
    }
}

#[tokio::test]
async fn dropping_the_call_stops_the_walk() {
    let dir = Temp(std::env::temp_dir().join(format!("miyu-walk-{}", std::process::id())));
    for name in ["a.txt", "sub/b.txt"] {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, "x").expect("写得进");
    }
    let everything = files(&dir.0, open(), &Stop::default(), |_| true);
    assert_eq!(everything.len(), 2, "没叫停的走得完");
    let (started, wait) = mpsc::channel();
    let (done, result) = mpsc::channel();
    let root = dir.0.clone();
    let task = tokio::spawn(blocking(Stop::default(), move |stop| {
        started.send(()).expect("还在等");
        let begin = Instant::now();
        while !stop.stopped() && begin.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let found = files(&root, open(), stop, |_| true);
        done.send((stop.stopped(), found.len())).expect("还在等");
    }));
    // 等的时候不占着跑异步任务的那一个线程：不然活根本派不出去。
    tokio::task::spawn_blocking(move || wait.recv_timeout(Duration::from_secs(10)))
        .await
        .expect("等得了")
        .expect("活开始了");
    task.abort();
    let (stopped, count) =
        tokio::task::spawn_blocking(move || result.recv_timeout(Duration::from_secs(20)))
            .await
            .expect("等得了")
            .expect("活停下了");
    assert!(stopped, "丢掉 future，旗就举起来");
    assert_eq!(count, 0, "举了旗，一步都不往下走");
}

/// 这一轮的配置：最终值是 `values`，没有密钥。
struct Turn(Values);

impl miyu_tool::SettingsPort for Turn {
    fn values(&self) -> Values {
        self.0.clone()
    }

    fn secret(&self, _: &Reference) -> Option<Secret> {
        None
    }
}

/// 人在配置里加的私密目录（施工 5-15 下）：往下走目录时同出厂的一样不进；没有配置端口的照出厂的。
#[test]
fn a_private_dir_added_in_the_settings_is_kept_out() {
    let dir = Temp(std::env::temp_dir().join(format!("miyu-walk-private-{}", std::process::id())));
    let diary = dir.0.join("home").join("diary");
    std::fs::create_dir_all(&diary).expect("建得了目录");
    let mut values = Values::default();
    values.set(
        miyu_tool::private::ADD,
        Value::List(vec![Value::Text(Cow::Borrowed("~/diary"))].into()),
    );
    std::fs::create_dir_all(dir.0.join("work")).expect("建得了目录");
    let mut call = miyu_tool::testkit::call("{}");
    // 工作目录在家目录外面：工作目录那一片里的照旧不挡。
    call.cwd = dir.0.join("work").to_string_lossy().into_owned();
    call.home = Some(dir.0.join("home"));
    call.settings = Some(Arc::new(Turn(values)) as Arc<dyn miyu_tool::SettingsPort>);
    let real = std::fs::canonicalize(&diary).expect("在");
    assert!(Fence::of(&call).keeps_out(&real), "加的私密目录不进");
    call.settings = None;
    assert!(!Fence::of(&call).keeps_out(&real), "没有配置的照出厂的");
}

/// 工作目录包住数据根、私密目录（施工 5-14 改：工作目录是 `~` 的也是工作区）：最深的那一片算，照样不进；工作目录落在数据根里的
/// （工作区 `workspace/`）那一片照进，数据根别处不进。
#[test]
fn a_working_dir_holding_the_data_root_still_keeps_it_out() {
    let dir = Temp(std::env::temp_dir().join(format!("miyu-walk-home-{}", std::process::id())));
    let home = dir.0.join("home");
    let data = home.join(".miyu");
    let own = data.join("home").join("admin").join("workspace");
    for path in [home.join(".ssh"), data.join("run"), own.clone()] {
        std::fs::create_dir_all(path).expect("建得了目录");
    }
    let mut call = miyu_tool::testkit::call("{}");
    call.cwd = home.to_string_lossy().into_owned();
    call.home = Some(home.clone());
    call.data_root = Some(data.clone());
    let real = |path: &std::path::Path| std::fs::canonicalize(path).expect("在");
    let fence = Fence::of(&call);
    assert!(fence.keeps_out(&real(&data.join("run"))), "数据根不进");
    assert!(fence.keeps_out(&real(&home.join(".ssh"))), "私密目录不进");
    assert!(!fence.keeps_out(&real(&home)), "家目录别处照进");
    call.cwd = own.to_string_lossy().into_owned();
    let fence = Fence::of(&call);
    assert!(
        !fence.keeps_out(&real(&own)),
        "工作目录在数据根里：那一片照进"
    );
    assert!(fence.keeps_out(&real(&data.join("run"))), "数据根别处不进");
}
