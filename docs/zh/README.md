# Miyu 文档

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

可以按你的情况选择从哪里读起：

| 你是 | 读 |
|---|---|
| 第一次使用 Miyu | [快速上手](guide/quickstart.md) |
| 正在使用 Miyu，想查一件事怎么做 | [用户手册](guide/README.md) |
| 想编写技能、脚本、软件包、扩展 | [作者教程](dev/packages.md) |
| 想写一个头（连接核心的界面程序），或者让其他程序连接核心 | [协议参考](reference/README.md)、[写一个头](dev/head.md) |
| 想弄懂 Miyu 的架构，或者给它提交改动 | [开发者文档](dev/README.md) |

## 文档放在哪

| 目录 | 是什么 | 给谁 |
|---|---|---|
| `zh/guide/`、`en/guide/` | 用户手册：安装、各个界面、会话、模型、权限、软件包、配置和排错。 | 使用 Miyu 的人 |
| `zh/reference/`、`en/reference/` | 协议参考：核心的每个方法，以及它的参数、回应和错误。 | 写头、写扩展的人 |
| `zh/dev/`、`en/dev/` | 开发者文档：架构、写一个头、主题和皮肤、往界面上放东西、贡献代码。 | 接入 Miyu、修改 Miyu 的人 |

上表里的目录都在仓库的 `docs/` 下。项目内部的图纸、设计和施工记录不在 `docs/` 里，放在仓库根目录的 `internal/` 下（`blueprint/`、`designs/`、`construction/`），是给维护者看的：使用 Miyu、给 Miyu 编写东西都不需要看它们；它们各是什么，见 [贡献代码](dev/contributing.md#先读什么)。

每种语言一个目录：`zh/` 是中文（正本），`en/` 是英文，两边的页面一一对应，页名相同。

`dev/` 和 `reference/` 使用 Apache-2.0 许可证（见仓库根目录的 `LICENSES/Apache-2.0.txt`）：你可以照着它们编写东西，使用什么许可证都可以。用户手册 `guide/` 和仓库里的其他内容一样，使用 AGPL-3.0-or-later。
