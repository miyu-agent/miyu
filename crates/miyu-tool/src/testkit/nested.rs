//! 假工具经脚本调工具的端口调（施工 M-2）。

use crate::{Call, Done, Nested};

/// 一件一件调 `calls`（工具名、参数原文），都了结了回一句成功，一行一件。
pub(super) async fn calls(call: &Call, calls: Vec<(&'static str, &'static str)>) -> Done {
    let Some(port) = call.nested.clone() else {
        return Done::error("no nested port");
    };
    let mut lines = Vec::new();
    for (name, args) in calls {
        let line = match port.call(name.to_string(), args.to_string()).await {
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
        };
        lines.push(line);
    }
    Done::ok(lines.join("\n"))
}
