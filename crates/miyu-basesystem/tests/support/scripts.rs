//! 场地里的脚本当一件工具（施工 X-12 上起；施工 X-44 从 `mod.rs` 挪出来）：照开头读、在 `work/` 里调一次。

use std::sync::Arc;

use miyu_tool::{Call, Done, Progress, Seen, Stop};

use super::{Site, resources};

impl Site {
    /// 把场地里的脚本 `file` 当一件工具（施工 X-12 上），在 `work/` 里调一次，参数是 `args`。
    pub async fn script(&self, file: &str, args: serde_json::Value) -> Done {
        self.script_as(file, args, None).await
    }

    /// 同 [`Site::script`]，没有参数，另交回跑的时候推给头的那几段（施工 X-45）。
    pub async fn script_progress(&self, file: &str) -> (Done, Vec<String>) {
        let tool = self.script_tool(file);
        let pushed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let into = Arc::clone(&pushed);
        let call = self.call_for("work", serde_json::json!({}), Seen::new(), Stop::default());
        let progress = Progress::new(move |text| into.lock().expect("没毒").push(text));
        let done = tool.run(call, progress).await;
        let pushed = pushed.lock().expect("没毒").clone();
        (done, pushed)
    }

    /// 同 [`Site::script`]，带上调用的事实 `caller`（施工 X-43）。
    pub async fn script_as(
        &self,
        file: &str,
        args: serde_json::Value,
        caller: Option<miyu_tool::Caller>,
    ) -> Done {
        let tool = self.script_tool(file);
        let call = Call {
            caller,
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool.run(call, Progress::new(|_| {})).await
    }

    /// 同 [`Site::script`]，会话的属主是 `owner`（施工 X-44：状态目录照它分），脚本的来处是 `source`。
    pub async fn script_by(&self, file: &str, source: &str, owner: &str) -> Done {
        let tool = self.script_tool_from(file, source);
        let call = Call {
            ids: Some(miyu_tool::CallIds {
                session: miyu_kernel::id::SessionId::parse("01a12f08-e382-70dc-942b-5deddc48d4da")
                    .expect("合写法"),
                owner: miyu_kernel::id::AccountId::parse(owner).expect("合写法"),
                call: miyu_kernel::id::CallId::new(
                    miyu_kernel::id::Seq::new(1).expect("合写法"),
                    1,
                )
                .expect("合写法"),
                asked: None,
                preset: None,
            }),
            ..self.call_for("work", serde_json::json!({}), Seen::new(), Stop::default())
        };
        tool.run(call, Progress::new(|_| {})).await
    }

    /// 场地里的 `file` 这个脚本当工具：照它的开头读，当家目录里放的。
    pub fn script_tool(&self, file: &str) -> Arc<dyn miyu_tool::Tool> {
        self.script_tool_from(file, miyu_store::scripts::HOME_PACKAGE)
    }

    /// 同 [`Site::script_tool`]，来处是 `source`（包的编号，施工 X-44）。
    pub fn script_tool_from(&self, file: &str, source: &str) -> Arc<dyn miyu_tool::Tool> {
        let path = self.0.join(file);
        let text = std::fs::read_to_string(&path).expect("读得出");
        let name = path
            .file_name()
            .expect("有名字")
            .to_string_lossy()
            .into_owned();
        let head = miyu_policy::script::read(&name, &text).expect("开头写对了");
        miyu_basesystem::script(&resources(), &head, path, source).expect("资源目录里的字读得出来")
    }
}
