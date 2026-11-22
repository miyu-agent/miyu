## 施工单 DOC-32：仓库首页和快速上手推荐 `litemiyu`

状态：做完了，等合并。项目主人 2026-11-23 让加的。

### 为什么

项目主人要在两个地方告诉读者：如果只使用终端界面，并且想要最轻量的程序，可以改用 `litemiyu`。快速上手里原来提到 `litemiyu` 的地方没有链接。

### 改了什么

加的这一句用的是项目主人给的原话，中英各一句：

- 中文：`如果你仅使用 TUI，并且想要最极致的轻量化设计，我推荐使用 [litemiyu](https://github.com/miyu-agent/litemiyu)。`
- 英文：`If you only use the TUI and want the most extreme lightweight design, I recommend [litemiyu](https://github.com/miyu-agent/litemiyu).`

| 文件 | 改动 |
|---|---|
| `README.zh-CN.md`、`README.md` | `# Miyu` 一节的介绍段后面另起一段，加上这一句 |
| `docs/zh/guide/quickstart.md`、`docs/en/guide/quickstart.md` | 「快速安装」一节里，在各系统的安装方法上面加上这一句；原来「还会装上极其轻量的 `litemiyu`」里的 `litemiyu` 加上指向仓库的链接 |

这四页是项目主人手写的页，除了他点名的这两处以外没有改动。

### 验收

- `python3 ~/.cache/miyu-docs-2026-11-14/lang/check.py .`：中文 62 页、英文 62 页，问题 0 个（中英两页行数相同）。
- 链接 `https://github.com/miyu-agent/litemiyu` 返回 200。
- 只改文档，没有改代码，没有运行门禁里的编译和测试。
