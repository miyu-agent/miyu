## 施工单 WD-8：安装脚本只认新格式的版本号

状态：做完了，等合并（分支 `step/WD-8-安装脚本只认新版本号`）。主会话 2026-11-22 派；项目主人对这件事的回复是「做一下过滤？」。

### 为什么

WD-7 修改安装脚本时触发了安装测试流程（`install-test`），三个平台都失败。原因不是 WD-7 的改动：

- `packaging/install.sh`、`install.ps1` 不加 `--prerelease` 时向 GitHub 询问 `releases/latest`。
- 仓库里还留着旧版 Miyu 的发行版，`releases/latest` 现在返回的是 2026-10-22 发布的 `v0.7.4`。它的包不是这个脚本安装得了的，也没有 `SHA256SUMS`。
- 脚本去下载 `v0.7.4` 的 `SHA256SUMS`，得到 404，报「下载失败」。测试第一步期望的是「还没有正式版，提示加 `--prerelease`」。

所以在第一个新格式的正式版发布之前，照手册不加参数运行安装脚本会失败在这一步。这个流程只在 `packaging/` 下的安装脚本和测试脚本变化时才触发，之前没有人看到。

### 改成什么样

两份脚本挑版本时一律询问发行版列表（`releases?per_page=100`，从新到旧），只认现在的版本号格式 `v<年>.<月>.<第几版>`（设计 59，例如 `v2026.11.0`、预发布 `v2026.11.0-rc.1`）：

- 标签不是四位年份开头的三段数字的，一律跳过。旧版的 `v0.7.x` 和早先演练用的 `v0.0.0-rc.N` 都属于这一类。
- 草稿跳过。
- 不加 `--prerelease` 时，标成预发布的、标签带 `-` 的也跳过，取剩下的第一个；一个都没有时停下，提示「还没有正式版；如果要安装预发布版，加上 --prerelease」，退出码 1，不安装任何东西。
- 加了 `--prerelease` 时，取列表里第一个新格式的版本（预发布也算）。
- `--version <版本>` 指定版本时不查询列表，行为没有变化。

`install.sh` 用 `tr` 加 `awk` 解析（不依赖 `jq`），`install.ps1` 直接筛 `Invoke-RestMethod` 交回的对象。给人看的句子没有改动，`install.ps1` 注释以外仍然只有 ASCII。

`miyu upgrade` 走的是另一套逻辑（`crates/miyu-cli/src/upgrade/plan.rs` 的 `newest`）：它在列表里取版本号最大的正式版，旧版的 `0.7.4` 比现在的版本小，只会得到「已是最新版本」，不会降级。这一步没有改动它。

### 验证

- 挑选逻辑单独拿出来，用仓库真实的发行版列表和五种假想的列表试过（`awk`、`gawk`）：

  | 列表 | 不加 `--prerelease` | 加 `--prerelease` |
  |---|---|---|
  | 现在的真实列表（`v2026.11.0-rc.2`、`rc.1`、`v0.0.0-rc.3`、`v0.7.4` …） | 没有 | `v2026.11.0-rc.2` |
  | 有了新的正式版（`v2026.12.0-rc.1`、`v2026.11.1`、`v2026.11.0`、`v0.7.4`） | `v2026.11.1` | `v2026.12.0-rc.1` |
  | 新的正式版还是草稿 | 没有 | `v2026.11.0-rc.2` |
  | 标签带 `-`、但没有标成预发布 | 没有 | `v2026.11.0-rc.3` |
  | 只有旧版 | 没有 | 没有 |
  | 空列表 | 没有 | 没有 |

- 在沙箱家目录里对着真实的 GitHub 运行 `install.sh`（不加参数）：

  ```
  $ HOME=<沙箱> LANG=zh_CN.UTF-8 sh packaging/install.sh
  miyu 安装：还没有正式版；如果要安装预发布版，加上 --prerelease
  （退出码 1，沙箱里没有装上任何东西）

  $ HOME=<沙箱> LANG=C sh packaging/install.sh
  miyu install: no stable release yet; add --prerelease to install a pre-release
  ```

- `sh -n packaging/install.sh` 通过。`install.ps1` 没有在本机运行，由 Windows 的安装测试流程验证。
- 验收标准：`install-test` 流程三个平台通过（装、核、卸一遍，资产用公开的预发布）。

### 连带的修改

- `packaging/test-install.ps1` 最后一条断言检查中文提示解得出来，期望的还是 WD-7 之前的「没有装着」。WD-7 把这一句改成了「未安装（… 不存在）」，当时安装测试流程因为上面的原因在第一步就停了，没有走到这一条；这一步改成期望「未安装」。

- 手册 `docs/zh/guide/install.md`、`docs/en/guide/install.md` 在参数表后面各加一句：不加参数时安装最新的正式版，还没有正式版时脚本停下并提示加 `--prerelease`。
