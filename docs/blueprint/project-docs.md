# 项目指令（施工 P-14）

仓库里的 `AGENTS.md`（没有的找 `CLAUDE.md`）回合开始作为一块事实交给她。设计：施工单 `docs/construction/P-14-读仓库里的AGENTS.md.md`（2026-10-20、10-23 项目主人定）。

## 文件

| 文件 | 管什么 |
|---|---|
| `crates/miyu-session/src/project_docs.rs` | 找哪些（`find`）、交不交（`tail`）、外壳（`render`）、出厂的字查一遍（`check`） |
| `crates/miyu-session/src/actor/project_docs.rs` | 回合开始调：本机的会话、预设开着的才找；工作目录 `~` 开头的照家目录换成真实路径 |
| `crates/miyu-store/src/resources/project_docs.rs` | 读 `core/project-docs/` 下的四份字 |
| `resources/core/project-docs/*.txt` | 给她看的字，登记在 `26-提示词.md` 第十节 |
| `resources/packages/basesystem/package.toml` | 功能 `project-docs`「项目指令」：没有工具，预设能开关 |

## 怎么走

1. **谁读**：本机的会话（场所会话一律不读）；预设里功能 `project-docs` 开着的（出厂的常规开发、全部功能开着，极简开发关着，2026-10-23 项目主人定）。
2. **找哪些**：工作目录在 git 仓库里的（往上找到有 `.git` 的那一层，目录、文件都算），从仓库根到工作目录逐层：每层先 `AGENTS.md`，没有再 `CLAUDE.md`；离工作目录越近的排得越后。不在仓库里的只找工作目录那一层（2026-10-23 项目主人定）。所有层共用 32 KiB 的总上限，超了截在字的边界上、末尾说一句（`cut.txt`）。读不了、不是 UTF-8 的当没有。
3. **什么时候交**：回合开始，和这一段上下文里模块 `project-docs` 交过的比。每块带 `refs`：`[内容的指纹, 仓库根]`。这个仓库没交过的交；交过但指纹不一样的交一份新的，头一句 `replaced.txt` 说替换的是哪个仓库的；一样的不交。压缩、撤销拿走了的，下一轮照没交过的再交。换仓库（shellhook 的会话每一轮换工作目录）第一次走进哪个就交哪个，同一个仓库里换子目录不重交。
4. **写法**：每份文件一层外壳 `open.txt`（`path` 是文件的真实路径，照常转义）、原文、`close.txt`；不进 system（文件一变整段重算、缓存就断），是 `context.injected` 的一块，`after` 是假，排在触发的那句前面。
5. **信任**：不设门（照 Codex、opencode）。内容是事实，不当 system 的规则；兜底的是权限和沙盒。

## 给她看的字

| 文件 | 原文 |
|---|---|
| `open.txt` | `<project-instructions path="{path}">` |
| `close.txt` | `</project-instructions>` |
| `replaced.txt` | `The project instructions below replace the earlier ones for {root}.` |
| `cut.txt` | `Project instructions stop here at {limit} bytes. Read the files for the rest.` |

## 第一版不做

- 第一次读子目录里的文件时补上那一层的（项目主人 10-23 排了，另一步）。
- `AGENTS.override.md`、`CLAUDE.local.md`、`@路径` 导入、全局的那一份。

## 测试

| 文件 | 测什么 |
|---|---|
| `crates/miyu-session/src/project_docs/tests.rs` | 仓库里逐层、先 AGENTS.md 后 CLAUDE.md；仓库外只找当层、没有的不交；总上限截在字边界、说一句；交过的一样的不交、变了换一份、别的仓库的另算；出厂的字填得进 |
| `crates/miyu-session/tests/project_docs.rs` | 真会话：第一轮交、第二轮不重交、改了换一份；场所会话不读 |
