# 项目指令（施工 P-14）

仓库里的 `AGENTS.md`（没有的找 `CLAUDE.md`）回合开始作为一块事实交给她。设计：施工单 `docs/construction/P-14-读仓库里的AGENTS.md.md`（2026-10-20、10-23 项目主人定）。

## 文件

| 文件 | 管什么 |
|---|---|
| `crates/miyu-session/src/project_docs.rs` | 找哪些（`find`）、交不交（`tail`）、外壳（`render`）、出厂的字查一遍（`check`） |
| `crates/miyu-session/src/actor/project_docs.rs` | 回合开始调：本机的会话、预设开着的才找；工作目录 `~` 开头的照家目录换成真实路径 |
| `crates/miyu-session/src/tools/docs.rs` | 子目录的（施工 P-14 下）：回合开始定下这一轮找不找（`Look`），工具结果送回来时补没交过的（`Docs::tell`），压缩、撤销、恢复清掉重来 |
| `crates/miyu-store/src/resources/project_docs.rs` | 读 `core/project-docs/` 下的四份字 |
| `resources/core/project-docs/*.txt` | 给她看的字，登记在 `26-提示词.md` 第十节 |
| `resources/packages/miyu-base-project-docs/package.toml` | 功能 `project-docs`「项目指令」：没有工具，预设能开关 |

## 怎么走

1. **谁读**：本机的会话（场所会话一律不读）；预设里功能 `project-docs` 开着的（出厂的常规开发、全部功能开着，极简开发关着，2026-10-23 项目主人定）。
2. **找哪些**：工作目录在 git 仓库里的（往上找到有 `.git` 的那一层，目录、文件都算），从仓库根到工作目录逐层：每层先 `AGENTS.md`，没有再 `CLAUDE.md`；离工作目录越近的排得越后。不在仓库里的只找工作目录那一层（2026-10-23 项目主人定）。所有层共用 32 KiB 的总上限，超了截在字的边界上、末尾说一句（`cut.txt`）。读不了、不是 UTF-8 的当没有。
3. **什么时候交**：回合开始，和这一段上下文里模块 `project-docs` 交过的比。每块带 `refs`：`[内容的指纹, 仓库根]`。这个仓库没交过的交；交过但指纹不一样的交一份新的，头一句 `replaced.txt` 说替换的是哪个仓库的；一样的不交。压缩、撤销拿走了的，下一轮照没交过的再交。换仓库（shellhook 的会话每一轮换工作目录）第一次走进哪个就交哪个，同一个仓库里换子目录不重交。
4. **写法**：每份文件一层外壳 `open.txt`（`path` 是文件的真实路径，照常转义）、原文、`close.txt`；不进 system（文件一变整段重算、缓存就断），是 `context.injected` 的一块，`after` 是假，排在触发的那句前面。
5. **信任**：不设门（照 Codex、opencode）。内容是事实，不当 system 的规则；兜底的是权限和沙盒。

## 子目录的（施工 P-14 下，项目主人 2026-10-23 排期）

1. **什么时候**：她读、改（`read`、`edit`、`write`，照工具结果的 `file.read`、`file.changed` 效果认，读技能的 `SKILL.md` 不算）工作目录下面某个子目录里的文件时。
2. **找哪些**：工作目录下面、到那个文件所在目录为止的每一层，每层先 `AGENTS.md`、没有再 `CLAUDE.md`，从外到里（`project_docs::below`）。工作目录那一层和以上回合开始交过，不算；不在工作目录下面的不找。工作目录先换成真实路径再比（效果里的是真实路径，macOS 的 `/tmp` 是链接）。在存效果的那个阻塞线程里找。
3. **补在哪**（定的）：接在那次工具结果后面，同一条结果里多一块（照 Claude Code），外壳同回合开始的（`open.txt`、`close.txt`）。不走模块事实：内核没有回合中途注入的入口，接在结果后面她当场看得见。
4. **交过的**（定的）：执行器记着每份文件交过的内容指纹（`Docs`），一样的不再交，变了的再交一份；一批落了盘的事件里有 `context.compacted`、`turn.reverted`、`turn.unreverted` 的整份清掉重来（交过的那几块可能已经不在她的上下文里）。核心重启以后从空的起，最多多交一次。
5. **上限**（定的）：每次补交单算 32 KiB（同回合开始那一块），超了截在字的边界上、末尾说一句（`cut.txt`）；截掉的那几份也记成交过（她读得到文件）。
6. **谁读**：同回合开始那一块，本机的会话、预设没关 `project-docs` 的；回合开始照会话定下 `Look`，场所会话、关了的这一轮不找。
7. 给她的字一句没加：外壳、截断那一句复用 `core/project-docs/`。

## 给她看的字

| 文件 | 原文 |
|---|---|
| `open.txt` | `<project-instructions path="{path}">` |
| `close.txt` | `</project-instructions>` |
| `replaced.txt` | `The project instructions below replace the earlier ones for {root}.` |
| `cut.txt` | `Project instructions stop here at {limit} bytes. Read the files for the rest.` |

## 第一版不做

- `AGENTS.override.md`、`CLAUDE.local.md`、`@路径` 导入、全局的那一份。

## 测试

| 文件 | 测什么 |
|---|---|
| `crates/miyu-session/src/project_docs/tests.rs` | 施工 P-14 下：工作目录下面到文件所在目录的每一层、工作目录那一层不算、不在下面的没有；仓库里逐层、先 AGENTS.md 后 CLAUDE.md；仓库外只找当层、没有的不交；总上限截在字边界、说一句；交过的一样的不交、变了换一份、别的仓库的另算；出厂的字填得进 |
| `crates/miyu-session/tests/project_docs.rs` | 真会话：第一轮交、第二轮不重交、改了换一份；场所会话不读；施工 P-14 下：读子目录里的文件，两层接在结果后面、从外到里，再读同一个子目录的不重交、根上那份不跟着重交，场所会话不补 |
| `crates/miyu-session/src/tools/docs/tests.rs`（施工 P-14 下） | 一样的交一次、变了再交；撤销以后清掉重来、别的事件不清；一次补交照上限截、截掉的也算交过 |
