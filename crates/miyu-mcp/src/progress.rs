//! 工具调用的进度（施工 PT-1 下，规范 2026-07-28「Progress」）：请求的 `_meta` 带 `progressToken`，服务照它推
//! `notifications/progress`，一条是做到多少、一共多少（可以没有）、一句说明（可以没有）。

use serde_json::Value;

/// 服务推来的一次进度。
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// 做到多少：规范要它每次都变大，没给总数的也是。
    pub progress: f64,
    /// 一共多少，没给的没有。
    pub total: Option<f64>,
    /// 给人看的一句，没给的没有。
    pub message: Option<String>,
}

impl Progress {
    /// 照通知的参数读；没有 `progress` 的不算。
    pub(crate) fn read(params: &Value) -> Option<Progress> {
        Some(Progress {
            progress: params.get("progress")?.as_f64()?,
            total: params.get("total").and_then(Value::as_f64),
            message: params
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    /// 给人看的一句：`3/10 Copying files`；没有总数的 `3 Copying files`，没有说明的只有数。整数不带小数点。
    pub fn said(&self) -> String {
        let number = |value: f64| {
            if value.fract() == 0.0 && value.abs() < 1e15 {
                format!("{value:.0}")
            } else {
                format!("{value}")
            }
        };
        let mut said = number(self.progress);
        if let Some(total) = self.total {
            said = format!("{said}/{}", number(total));
        }
        if let Some(message) = self
            .message
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            said = format!("{said} {}", message.trim());
        }
        said
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Progress;

    #[test]
    fn progress_reads_and_says_itself() {
        let full = Progress::read(
            &json!({"progressToken": "p1", "progress": 3, "total": 10.0, "message": " Copying "}),
        )
        .expect("读得成");
        assert_eq!(full.said(), "3/10 Copying");
        let bare = Progress::read(&json!({"progress": 0.5})).expect("读得成");
        assert_eq!(bare.said(), "0.5");
        assert!(
            Progress::read(&json!({"total": 3})).is_none(),
            "没有 progress 的不算"
        );
    }
}
