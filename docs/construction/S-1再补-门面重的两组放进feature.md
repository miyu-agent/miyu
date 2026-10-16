## 施工单 S-1 再补：门面重的两组放进 feature

状态：做好了（2026-10-17，本机门禁八项；门面默认 116 个 crate、打开两个 feature 193 个，重的几样默认一个都不拉；变异 2 处都抓住）。网页的会话钉 68457b03 之前量出来的：门面 `miyu-client` 从 S-6 上起无条件依赖 `miyu-http`、`miyu-tool`，网页一样都不用，却多拉了 81 个 crate（reqwest、rustls、ring、quinn、`miyu-sandbox`、landlock、一整套 icu）；网页的 release 程序从 4,798,304 涨到 5,149,968 字节（多 351 KB，+7.3%），debug 版从 172 MB 涨到 205 MB。图纸 `docs/blueprint/client.md`「规矩」。

### 这一步做什么

1. `miyu-http`、`miyu-tool` 改成门面的可选依赖：`tooltext` 一组放在 feature `tool` 后面，`http` 一组放在 feature `http` 后面，都默认不开。
2. 接入QQ（`crates/miyu-onebot`）依赖门面时打开这两个；界面什么都不用改。
3. 门面测试：两组照 feature 分开钉住；另查这两个依赖是可选的、没有默认打开的 feature。

### 验收

- `cargo xtask check` 八项。
- 两个头换到新的提交以后照原样编 release，大小回到钉 S-6 上之前的样子；网页数一下 rustls、reqwest、ring、landlock 的符号还剩几个。
