//! 假工具经脚本调工具的端口调（施工 M-2）。

use std::sync::{Arc, Mutex, PoisonError};

use crate::{Call, Done, Effect, Nested};

/// 一件一件调 `calls`（工具名、参数原文），都了结了回一句成功，一行一件。像真的 `shell` 那样交「在等人」的口子（施工 M-7
/// 上），收到过的照先后接一行 `held true,false`；没收到的不接。
pub(super) async fn calls(call: &Call, calls: Vec<(String, String)>, command: bool) -> Done {
    let Some(port) = call.nested.clone() else {
        return Done::error("no nested port");
    };
    let heard = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&heard);
    port.hold(Box::new(move |held| {
        record
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(held.to_string());
    }));
    let mut lines = Vec::new();
    for (name, args) in calls {
        let nested = match command {
            true => port.command(name.clone(), args).await,
            false => port.call(name.clone(), args).await,
        };
        let line = line(&name, nested);
        lines.push(line);
    }
    let held = heard
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .join(",");
    if !held.is_empty() {
        lines.push(format!("held {held}"));
    }
    Done::ok(lines.join("\n"))
}

/// [`crate::testkit::Act::Commands`] 这一次调哪几次：参数 `{"commands": [[<工具>, <参数原文>], …]}`；不是这样的没有。
pub(super) fn commands(args: &str) -> Option<Vec<(String, String)>> {
    let value: serde_json::Value = serde_json::from_str(args).ok()?;
    value
        .get("commands")?
        .as_array()?
        .iter()
        .map(|pair| {
            let pair = pair.as_array()?;
            Some((
                pair.first()?.as_str()?.to_string(),
                pair.get(1)?.as_str()?.to_string(),
            ))
        })
        .collect()
}

/// 一次调用的结局写成一行：`<工具> <结局>`，跑完的再写字和 `data`，丢了图的写 `images dropped`，拒绝的写原因。
fn line(name: &str, nested: Nested) -> String {
    match nested {
        Nested::Refused(refusal) => format!("{name} refused {refusal:?}"),
        Nested::Settled(settled) => {
            let mut line = format!("{name} {}", settled.status.as_str());
            if !settled.text.is_empty() {
                line.push_str(&format!(" text={}", settled.text));
            }
            if let Some(data) = &settled.data {
                line.push_str(&format!(" data={data}"));
            }
            if let Some(reason) = &settled.reason {
                line.push_str(&format!(" reason={reason}"));
            }
            if settled.images_dropped {
                line.push_str(" images dropped");
            }
            line
        }
    }
}

/// 放到后台交上了的（结果里报了 `job.started`）：端口挂到那个任务下面，等 `wait` 毫秒在旁边调一次 `name`，答回来的记进
/// `heard`。
pub(super) fn aside(
    call: &Call,
    done: &Done,
    name: &'static str,
    heard: Arc<Mutex<Vec<String>>>,
    wait: u64,
) {
    let job = done.effects.iter().find_map(|effect| match effect {
        Effect::JobStarted(started) => Some(started.job.clone()),
        _ => None,
    });
    let (Some(port), Some(job)) = (call.nested.clone(), job) else {
        return;
    };
    port.adopt(job);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
        let line = line(name, port.call(name.to_string(), "{}".to_string()).await);
        heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line);
    });
}
