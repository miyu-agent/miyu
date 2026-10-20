## 施工单 T-12 补：常用供应商加 ClinePass

状态：做完（2026-10-21，项目主人要的；施工分支 `step/T-12补-ClinePass`）。

### 做什么

- `resources/models/featured.toml` 加 ClinePass（Cline 的包月），排在 OpenCode Go 后面：三家包月的排在一起。`miyu setup` 和头的第一次引导列的是同一份，不改代码。
- 出厂的模型目录（models.dev 的快照）里本来有它：编号 `cline-pass`，地址 `https://api.cline.bot/api/v1`，OpenAI 兼容，key 的环境变量 `CLINE_API_KEY`，模型名自己带 `cline-pass/` 前缀（引用写成 `cline-pass/cline-pass/<模型>`，切在第一个 `/`，8-6 真模型那次照这个形状跑通过）。不另写档案。
- 图纸 `models.md` 的先后、`cli/setup.md` 的样子（编号顺延一位）。

### 验收

1. `miyu-core` 的 `every_featured_provider_is_usable_with_the_bundled_data`：15 家拿出厂的目录、档案都接得上。
2. `cargo xtask check` 八项。
3. 真 key 没有（局域网那个网关不是官方地址），没走真请求；项目主人有 ClinePass 的 key 时 `miyu setup` 选它试一次。
