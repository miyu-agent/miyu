## 施工单 DOC-12：报告安全问题的渠道，打包说明去掉施工号

状态：待验收（2026-11-18）。项目主人定的两件（主会话转来）：开 GitHub 的私密漏洞报告，加 `SECURITY.md`、手册那句跟着改；`packaging/README.md` 发版前整篇清掉施工号。

### 改了什么

- `SECURITY.md`（新，仓库根，中文在前、英文在后）：私下报告，不开公开的 issue、PR；在仓库的 Security 页点 Report a vulnerability；别的仓库的问题也报到这里；报告里写版本和系统、复现步骤、后果；挡什么不挡什么指到手册的安全页。没有写多久回复、怎么公告：这些没定，不编。
- `guide/security`「发现安全问题」（中英）：原来只能写「先说想私下报告，等项目回复联系的办法」，换成去哪里报、指到 `SECURITY.md`。
- `packaging/README.md`：去掉 13 处施工号和两处讲来历的话（「第一版的重点」「原来打算的第二个本机入口随多用户一起不做了」「不再空闲 10 分钟就退」）；指设计、施工单的地方写成仓库里的文件路径。命令、路径、文件名一个没动。
- `docs/construction/DOC-2-英文版.md` 一处：示例里的 `[<页>.en.md](<页>.en.md)` 加上反引号。它照 CommonMark 本来就不成链接，外部审计当成了断链，加上免得再被认错。

### 实测

- `gh api repos/miyu-agent/miyu/private-vulnerability-reporting` 现在回 `{"enabled":false}`：开关还没开，`SECURITY.md` 和手册里那个地址（`…/security/advisories/new`）要等项目主人在仓库设置里点开以后才通。开了以后用另一个账号点一遍「Report a vulnerability」看到表单才算验过，这一步没法替。
- `packaging/README.md` 只删字：改前改后对比，代码块、反引号里的命令和路径逐个相同。
- 中英对齐、锚点、链接三个脚本照基线过（38 页，问题 0；链接基线 12 条不变，`../../SECURITY.md` 指得到）。

### 查到、没改的

- 核心仓库的 Issues 是关着的（`gh issue list` 回 `has disabled issues`），而 `guide/troubleshooting`「还是不行」让人到 `https://github.com/miyu-agent/miyu/issues` 提。开不开 Issues 要项目主人定，已交主会话；定了以后这一句跟着改或者不用改。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. `packaging/README.md` 里搜不到「施工」。
3. 私密漏洞报告开了以后，`SECURITY.md` 里的地址打得开。
