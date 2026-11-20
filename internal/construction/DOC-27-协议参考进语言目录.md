## 施工单 DOC-27：协议参考搬进 docs/zh/reference、docs/en/reference

状态：待验收（2026-11-20）。项目主人定整理文档：文档按语言分目录。协议参考原来中英并排放在 `docs/reference/`（英文叫 `<页>.en.md`），和用户手册、开发者文档的放法不一样。文档大搬家三步里的第二步。

### 改了什么

- `docs/reference/<页>.md` 搬到 `docs/zh/reference/<页>.md`，`docs/reference/<页>.en.md` 搬到 `docs/en/reference/<页>.md`，各 20 页，页名两边相同。
- `docs/reference/methods-v1.txt`（协议 1 冻结的方法表，门禁用的快照，不是文档）搬到 `internal/blueprint/methods-v1.txt`。
- 链接和路径跟着改：相对链接 163 处（手册、开发者文档、作者教程指进来的，参考各页之间的 `.en.md`），代码注释和内部文档里写明的路径 147 处。
- 每页开头指到另一种语言的那一句去掉（39 处），标题下面加署名行，和别的页一致。
- 门禁（xtask）：
  - `reference.rs`：原来读一个目录、靠 `.en.md` 后缀分中英，改成分别读 `docs/zh/reference`、`docs/en/reference`；报错里写的是哪个目录。
  - `anchors.rs`：两个目录各查一遍锚点。
  - `frozen.rs`：快照的路径换成新位置。
- 两处测试读文件的路径跟着换：`miyu-endpoint` 里核对出错原话的测试读中文的 `errors.md`，核对方法表的测试读新位置的快照。
- 说明跟着改：`AGENTS.md`、两份文档总目录、开发者文档目录、架构页和贡献代码里列目录的地方、蓝图 `licenses.md` 里 Apache-2.0 管哪些目录的那一句（目录换了，范围没变）。

### 顺带查出来的

文档检查要求中英两份行数、表格相等，搬进来以后查出英文会话页（`sessions.md`）两处落后于中文，照中文补上：

- 会话列表每项的 `default_workspace` 那一行放错了表（放在了新建会话的参数表里），挪回去，那张表几行的先后照中文排。
- `session.answer` 的出错表少一行 `unknown_attachment`、`attachment_too_big`，`bad_params`、`unexpected_reason` 两行少了讲附件的半句。

### 没做的

- 协议参考的写法还没有照「文档的规矩」改：句子是内部文档式的省略短句，称 AI 用了「她」（中文 69 处、英文 she/her 92 处，文档检查还报着英文 12 页）。这要逐页改写，另排一步。
- 作者教程还在 `docs/authors/`，下一步（DOC-28）并进开发者文档。

### 核实

- `cargo xtask check`：头一遍只有 clippy 一项没过（`reference.rs` 里读目录的函数返回类型太复杂，原来靠 `allow` 标注，改成起一个类型别名），别的十一项全过，「文档」一项里协议参考、锚点、冻结的方法表三样都读新位置；改完单跑 xtask 的 clippy 和 137 个测试，过了。
- 文档线的检查脚本：中文 57 页、英文 57 页一页对一页，1479 个链接都指得到；剩下的 12 个问题都是上面说的英文页里的 she/her。
- 全仓库搜 `docs/reference`：只剩施工单里的历史记录。

### 蓝图改哪几节

`licenses.md`「对外的样子」里 Apache-2.0 管的目录：`docs/reference/` 换成 `docs/zh/reference/` 和 `docs/en/reference/`。

### 验收

1. `docs/` 下没有 `reference/`；`docs/zh/reference/`、`docs/en/reference/` 各 20 页，页名相同。
2. `cargo xtask check` 全过，三台 CI 全绿。
3. 新加一个协议方法时照旧：`cargo xtask methods` 登记进 `internal/blueprint/methods-v1.txt`，中英两个目录各补条目，漏了门禁报红。
