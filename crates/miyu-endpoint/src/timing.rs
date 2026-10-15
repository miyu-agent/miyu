//! 核心等多久的几样（施工 X-16 从 `lib.rs` 挪出来）：握手、文件清单、分块上传、一次性码、扩展进程和 MCP 服务的时长。测试里照
//! 这几个设短的，不用真等；核心起来时都照默认的。

use std::time::Duration;

use crate::{Core, extensions, login, mcp};

impl Core {
    /// 同一份家底，连上以后最多等 `wait` 握手：测试里设短的，不用真等 10 秒。
    #[must_use]
    pub fn with_hello_wait(mut self, wait: Duration) -> Core {
        self.hello_wait = wait;
        self
    }

    /// 同一份家底，`fs.find` 的 `fresh` 照 `fresh` 这个时长判断要不要重建清单（施工 W-2）：测试里设短的，不用
    /// 真等十秒。
    #[must_use]
    pub fn with_files_fresh(mut self, fresh: Duration) -> Core {
        self.files_fresh = fresh;
        self
    }

    /// 同一份家底，分块上传多久没写就作废照 `idle`（施工 W-5）：测试里设短的，不用真等 60 秒。
    #[must_use]
    pub fn with_upload_idle(mut self, idle: Duration) -> Core {
        self.upload_idle = idle;
        self
    }

    /// 同一份家底，一次性码 `ttl` 有效（施工 W-8）：测试里设短的，不用真等 5 分钟。
    #[must_use]
    pub fn with_code_ttl(mut self, ttl: Duration) -> Core {
        self.identity = login::Identity::new(ttl);
        self
    }

    /// 同一份家底，扩展进程、MCP 服务（施工 X-3 上）等多久、退避多久照 `timing`（施工 9-4 上）：测试里设短的，不用真等。
    #[must_use]
    pub fn with_extension_timing(mut self, timing: extensions::Timing) -> Core {
        self.extensions = extensions::Extensions::new(timing);
        self.mcp = mcp::Servers::new(timing);
        self
    }
}
