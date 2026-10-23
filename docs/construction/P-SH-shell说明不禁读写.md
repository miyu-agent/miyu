## 施工单 P-SH：shell 说明不禁读写

状态：合了（2026-10-23）。项目主人定（验收调查会话转来）。

### 为什么

`shell` 的说明第二句写死了「Use it for builds, tests, git and other programs, not to read, search or edit files.」：

1. 不看实际开了哪些工具：只开 `shell`、`read` 时她没有别的办法改文件，这句和工具面矛盾。
2. 读写工具都开着也不该是「不准」：用 grep、sed、heredoc、python 读、搜、批量改文件是正常做法，有时更合适。这几次 SVG 测试她全靠 shell 写文件、改文件，零出错、成品最好。
3. 真要她优先用专用的工具，偏好写在那件工具自己的说明里、只在它开着时出现（`grep` 的说明本来就写着「Prefer this over grep or rg in the shell」），不放进 `shell` 写成禁令。

### 做了什么

- `resources/software/basesystem/tools/shell.json`：第二句只留「Use it for builds, tests, git and other programs.」。
- 登记簿 `26-提示词.md`：那一行的 token、为什么、指纹，另记一条量法；`cargo xtask prompts` 重生成 `docs/blueprint/prompts.md`；蓝图 `tools/shell.md` 的引文。
- token：扩展线照开发端点量，资源文件原样十八件的 tools 数组 3166 → 3157，`shell` 的边际份量 239 → 230（-9）。

### 怎么验

- 任意工具组合下，会话快照里 `shell` 的说明都没有这一句（说明是常量，只这一处）。
- 只开 `shell`、`read` 时，各件说明之间没有互相矛盾的（`read` 只说怎么读，`grep`、`edit` 不在工具面上就不出现）。
- `cargo xtask check`。
