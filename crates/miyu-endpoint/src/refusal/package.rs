//! 装、卸软件包时的三种拒绝（施工 F-5 上，`packages.md`「装卸」）：`refusal.rs` 到了 500 行，挪到这里。施工 F-6 上多开关的两种
//! （`package-pages.md`「开关」「程序不在就当没装」）。扩展带数据的几种（工具规格不对、能力没批、没导出、embedding 算不出）施工
//! X-19 时 `refusal.rs` 又到了 500 行，也挪到这里。

use miyu_store::packages::conflicts::Holder;

use super::{REFUSED, Refusal};

impl Refusal {
    /// 装的包和出厂的同一个编号（施工 F-5 上）。
    pub(crate) const PACKAGE_EXISTS: Refusal = Refusal {
        code: REFUSED,
        reason: "package_exists",
        data: None,
    };
    /// 包文件解不开（施工 G-1）：不是 zstd 压的 tar、有链接、路径跑到外面、太大。
    pub(crate) const PACKAGE_FILE_BAD: Refusal = Refusal {
        code: REFUSED,
        reason: "package_file_bad",
        data: None,
    };
    /// 清单写了 `targets`，里面没有本机这一种系统（施工 G-1）。
    pub(crate) const WRONG_TARGET: Refusal = Refusal {
        code: REFUSED,
        reason: "wrong_target",
        data: None,
    };
    /// 卸必需的包（施工 F-5 上）：基础系统。
    pub(crate) const PACKAGE_REQUIRED: Refusal = Refusal {
        code: REFUSED,
        reason: "package_required",
        data: None,
    };
    /// 包的程序不在 `miyu` 旁边（施工 F-6 上）：当没装，开不了。
    pub(crate) const PROGRAM_MISSING: Refusal = Refusal {
        code: REFUSED,
        reason: "program_missing",
        data: None,
    };
    /// 这个包没有开关（施工 F-6 上）：必需的、界面、小程序。
    pub(crate) const NOT_SWITCHABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "not_switchable",
        data: None,
    };
    /// 个人设置里没有这个名字的 MCP 服务（施工 F-9 补，`mcp.restart`）。
    pub(crate) const UNKNOWN_SERVER: Refusal = Refusal {
        code: REFUSED,
        reason: "unknown_server",
        data: None,
    };
    /// 个人设置里没写这个名字、有包带着同名的 MCP 服务（施工 X-26，`mcp.set`）：随包来的不经它改。
    pub(crate) const SERVER_FROM_PACKAGE: Refusal = Refusal {
        code: REFUSED,
        reason: "server_from_package",
        data: None,
    };
    /// 没有这个名字的技能（施工 W-13，`skill.remove`）：`data.name`。
    pub(crate) fn unknown_skill(name: &str) -> Refusal {
        Refusal::with("unknown_skill", "name", serde_json::json!(name))
    }
    /// 随包来的技能不能删（施工 W-13，`skill.remove`）：`data.name`。
    pub(crate) fn skill_from_package(name: &str) -> Refusal {
        Refusal::with("skill_from_package", "name", serde_json::json!(name))
    }
    /// 重启关着的包（施工 X-4，`package.restart`）。
    pub(crate) const PACKAGE_OFF: Refusal = Refusal {
        code: REFUSED,
        reason: "package_off",
        data: None,
    };
    /// 这个包没有能重启的进程、服务（施工 X-4）：内置包、界面、小程序、吉祥物包。
    pub(crate) const NOT_RESTARTABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "not_restartable",
        data: None,
    };
    /// 这个包没有后台页（施工 F-6 中，`package-pages.md`「`package.file`」）。
    pub(crate) const NO_PAGE: Refusal = Refusal {
        code: REFUSED,
        reason: "no_page",
        data: None,
    };
    /// 后台页里没有这份文件、跑出了后台页的目录、不是普通文件（施工 F-6 中）。
    pub(crate) const FILE_NOT_FOUND: Refusal = Refusal {
        code: REFUSED,
        reason: "not_found",
        data: None,
    };
    /// 包的程序没连着：没开、停了、还没登记方法（施工 F-6 中，`package.call`）。
    pub(crate) const NOT_RUNNING: Refusal = Refusal {
        code: REFUSED,
        reason: "program_not_running",
        data: None,
    };
    /// 程序到时限没回（施工 F-6 中）。
    pub(crate) const METHOD_TIMEOUT: Refusal = Refusal {
        code: REFUSED,
        reason: "method_timeout",
        data: None,
    };
    /// 程序没登记这个方法（施工 F-6 中）：`data.method` 是哪一个。
    pub(crate) fn unregistered(method: &str) -> Refusal {
        Refusal::with(
            "unregistered",
            "method",
            serde_json::Value::String(method.to_string()),
        )
    }
    /// 程序回了错（施工 F-6 中）：`data.message` 是它说的那一句，`data.code` 是它的错误码，没有的不写。
    pub(crate) fn method_failed(message: String, code: Option<serde_json::Value>) -> Refusal {
        let mut refusal = Refusal::with(
            "method_failed",
            "message",
            serde_json::Value::String(message),
        );
        if let (Some(data), Some(code)) = (&mut refusal.data, code) {
            data.insert("code".to_string(), code);
        }
        refusal
    }
    /// 要装的包和装着的别的包重了名（施工 F-8 六补，`packages.md`「冲突」）：`data.conflicts` 一处一条
    /// `{"kind", "name", "package"}`，照哪一样、名字、包排。
    pub(crate) fn package_conflict(
        conflicts: &[miyu_store::packages::conflicts::Conflict],
    ) -> Refusal {
        let list = conflicts
            .iter()
            // 撞了装着的包的写 `package`，撞了人格、预设的一层的写 `layer`（施工 P-11 下）。
            .map(|one| match &one.holder {
                Holder::Package(package) => {
                    serde_json::json!({"kind": one.kind.as_str(), "name": one.name, "package": package})
                }
                Holder::Layer(layer) => {
                    serde_json::json!({"kind": one.kind.as_str(), "name": one.name, "layer": layer.as_str()})
                }
            })
            .collect();
        Refusal::with(
            "package_conflict",
            "conflicts",
            serde_json::Value::Array(list),
        )
    }
    /// 装的包依赖的包没装（施工 F-11）：`data.missing` 是缺的那几个，照清单写的先后。
    pub(crate) fn depends_missing(missing: &[&str]) -> Refusal {
        Refusal::with("depends_missing", "missing", serde_json::json!(missing))
    }

    /// 卸的包被别的装着的包依赖着（施工 F-11）：`data.by` 是依赖它的那几个，照编号。
    pub(crate) fn package_depended(by: &[String]) -> Refusal {
        Refusal::with("package_depended", "by", serde_json::json!(by))
    }

    /// 包文件哪一处都没下成（施工 G-4 上）：`data.why` 是每一处的原因（英文，带地址），照先后用 `; ` 接起来。
    pub(crate) fn download_failed(why: String) -> Refusal {
        Refusal::with("download_failed", "why", serde_json::Value::String(why))
    }

    /// 软件源一处都没拿成、本机也没有索引（施工 G-3 上）：`data.why` 是最后一处的原因（英文原话，带地址）。
    pub(crate) fn source_unreachable(reason: String) -> Refusal {
        Refusal::with(
            "source_unreachable",
            "why",
            serde_json::Value::String(reason),
        )
    }

    /// 装的清单写错了、和别的包撞了（施工 F-5 上）：`data.problem` 照连接的语言说一句，知道第几行的带 `data.line`。
    pub(crate) fn package_invalid(problem: String, line: Option<usize>) -> Refusal {
        let mut refusal = Refusal::with(
            "package_invalid",
            "problem",
            serde_json::Value::String(problem),
        );
        if let (Some(data), Some(line)) = (&mut refusal.data, line) {
            data.insert("line".to_string(), serde_json::json!(line));
        }
        refusal
    }

    /// `mcp.set` 交来的服务写错了（施工 X-26）：`data.problem` 照连接的语言说哪里不对。
    pub(crate) fn mcp_invalid(problem: String) -> Refusal {
        Refusal::with("mcp_invalid", "problem", serde_json::Value::String(problem))
    }

    /// `provide` 的一件工具规格不对、撞名（施工 O-2 上）：`data.tool` 是哪一件，`data.problem` 是哪一条。
    pub(crate) fn bad_tool(tool: &str, problem: &str) -> Refusal {
        let mut refusal = Refusal::with("bad_tool", "tool", serde_json::json!(tool));
        if let Some(data) = refusal.data.as_mut() {
            data.insert("problem".to_string(), serde_json::json!(problem));
        }
        refusal
    }

    /// `model.embed` 算不出：哪一条没算出来、等不到（施工 X-17），`data.why` 是原因（英文一句）。
    pub(crate) fn embedding_failed(why: &str) -> Refusal {
        Refusal::with(
            "embedding_failed",
            "why",
            serde_json::Value::String(why.to_string()),
        )
    }

    /// 扩展经 `package.call` 调了对方清单 `[export]` 里没有的方法（施工 X-18）：`data.method` 是哪一个。
    pub(crate) fn not_exported(method: &str) -> Refusal {
        Refusal::with(
            "not_exported",
            "method",
            serde_json::Value::String(method.to_string()),
        )
    }

    /// 开一个扩展，它要的能力还有没批的（施工 9-4 下上，`extensions.md`「能力」）：`data.capabilities` 是没批的那几个。
    pub(crate) fn needs_approval(capabilities: serde_json::Value) -> Refusal {
        Refusal::with("needs_approval", "capabilities", capabilities)
    }
}
