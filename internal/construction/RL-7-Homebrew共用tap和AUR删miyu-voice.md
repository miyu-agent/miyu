## 施工单 RL-7：Homebrew 配方放进共用的 tap，AUR 的 `miyu-voice` 提删除请求

状态：做完了，等合并。项目主人 2026-11-28 定的两件，只改文档和打包模板。

### 为什么

`internal/发版手册.md` 第十五节「待核实」的第 1、3 条：

1. 模板、安装文档写的 tap 是 `miyu-agent/homebrew-miyu`，这个仓库不存在；组织里有的是 LiteMiyu 在用的 `miyu-agent/homebrew-tap`。主人定：Miyu 的配方也放进 `miyu-agent/homebrew-tap`，装法 `brew tap miyu-agent/tap`、`brew install miyu-agent/tap/<配方名>`，配方名照 `packaging/homebrew/` 现有的。
2. AUR 的旧版语音包 `miyu-voice` 依赖 `miyu=0.7.4-1`，过渡包 `miyu` 升到 `2026.11.0` 以后装着它的人 `-Syu` 会被依赖挡住。主人定：在 AUR 上提删除请求，不做过渡包，也不只标孤包；旧用户先自己卸掉它，写进正式版的发版说明。

### 改了什么

| 文件 | 改动 |
|---|---|
| `packaging/homebrew/*.rb.in`（五份）、`packaging/test-brew.sh` | 注释里的 tap 仓库改成 `miyu-agent/homebrew-tap`；`miyu-agent.rb.in` caveats 里装托盘的命令改成 `brew install --cask miyu-agent/tap/miyu-applet` |
| `docs/zh/guide/install.md`、`docs/en/guide/install.md` | Homebrew 一节：tap 是 `miyu-agent/tap`，和 LiteMiyu 共用；三条命令带上 tap 前缀 |
| `internal/designs/59-发版.md` | 第三节第 3 条的 Homebrew 一点写上 tap 名；`miyu-voice`「标成孤包」改成提删除请求；第九节加 2026-11-28 一段 |
| `internal/designs/60-系统源分包.md` | 第五节第 2 条写上 tap 名 |
| `internal/发版手册.md` | 第二节的表填上 tap；第十一节第 2 项（发版说明）加 Homebrew 装法和「先卸掉 `miyu-voice`」两条；第 4 项第 5 步写成可照做的命令；加第 6 步「给 `miyu-voice` 提删除请求」；第十五节待核实第 1、3 条改成已定 |
| `internal/construction/RL-5-系统源打包.md` | 「发版那天」里的 tap 名、装法跟着改，标明 2026-11-28 改 |

删除请求为什么只能在网页上提：AUR 的 SSH 接口没有提请求的命令。2026-11-28 查了 aurweb 源码（GitHub 镜像 `archlinux/aurweb`，提交 `ed420d4`）：`aurweb/git/serve.py` 的命令只有 `adopt`、`disown`、`flag`、`unflag`、`vote`、`unvote`、`set-comaintainers`、`set-keywords`、`list-repos`、`restore`、`help` 和 git 的两个；请求在 `aurweb/routers/pkgbase.py` 的网页表单 `/pkgbase/<名字>/request`，类型有 deletion、merge、orphan、adoption，留言必填；只有建了不到 24 小时（`auto_delete_age = 86400`）、维护者自己提的删除请求自动通过。AUR 的接口 2026-11-28 回 `miyu-voice` 是 `0.7.4-1`，依赖 `miyu=0.7.4-1`，维护者 `shorin`。

### 不做什么

- 不动 `miyu-agent/homebrew-tap` 仓库，正式版那天照手册第十一节第 4 项第 5 步加文件。
- 不提 AUR 请求，正式版那天照第 6 步由主人提。
- 快速上手（`docs/zh/guide/quickstart.md`、`docs/en/guide/quickstart.md`）第 15 行还写着 `brew tap miyu-agent/miyu`。这两页是主人手写的，没有改，留给主人定。
- 旧 tap `SHORiN-KiWATA/homebrew-miyu` 的 `miyu` 配方改成什么样，还是待核实（手册第十五节第 2 条）。

### 验收

- `git grep -n homebrew-miyu` 只剩旧 tap `SHORiN-KiWATA/homebrew-miyu` 和设计 59 第九节的来历。
- `git grep -n "miyu-agent/miyu/"` 没有 Homebrew 的装法。
- 门禁全过（只改文档和模板注释，编译和测试结果应当不变）。
