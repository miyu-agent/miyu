## 施工单 SCRUB-4：存档目录名和渠道前缀

状态：做完了（2026-11-20），等验收。SCRUB-3 复核留下的两件小事。

### 做什么

1. **门禁拦完整的会话存档目录名**（`xtask/src/privacy.rs`）：原来取 `-home-` 后面整串名字字符再看结尾是不是 `-Documents`，名字字符里有 `-`，`-home-<名字>-Documents-github-…` 这种完整的存档目录名取到的是整串，结尾对不上，漏了。改成取 `-home-` 后面到第一个 `-` 为止的那段比白名单，后面紧跟 `-Documents` 才算。补一个测试，改前红。
2. **渠道前缀**：文档里 12 行还写着 `<渠道前缀>/cline-pass/…`，照 SCRUB-3 的做法去掉渠道前缀，留公开的 `cline-pass/…` 编号。登记簿 `internal/designs/26-提示词.md` 里的一行先改，`prompts.md` 用 `cargo xtask prompts` 重新生成。

### 验收

1. `cargo test -p xtask privacy`：新测试通过，改前红。
2. `cargo xtask check` 12 项全 ✓。
3. 在 `docs/`、`internal/` 里搜渠道前缀，没有剩的。
