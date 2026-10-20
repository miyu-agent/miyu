# Scripts, skills, command templates

None of these need a long-running program; one file is enough. Chinese version: [scripts-skills.md](scripts-skills.md).

| | What it is | How the AI uses it |
|---|---|---|
| **Script** | An executable file; one tool | Called like any other tool |
| **Skill** | A `SKILL.md`; a way of doing something | Normally sees only its name and one line; reads the body when it needs it |
| **Command template** | A `<name>.md`; a slash command | When a person types `/<name>`, the body is sent to the AI as that person's message |

Where: in a package, under the package folder's `scripts/`, `skills/`, `commands/`, with a manifest of just `[package]`; just for you, straight into `~/.miyu/home/admin/scripts/`, `skills/`, `commands/` (same names there win). The next session sees them; each script, skill and template is switched separately in presets. Broken ones aren't registered, and `miyu check` says which and why.

```text
dice/
├── package.toml          # only [package]
├── scripts/
│   └── dice.py
├── skills/
│   └── take-notes/
│       └── SKILL.md
└── commands/
    └── review.md
```

```toml
[package]
name = { en = "Dice and notes", zh = "骰子和笔记" }
```

## Scripts

One file, one tool. The tool name is the file name (extension removed, characters outside `[A-Za-z0-9_-]` become `_`); a comment header gives the description and parameters. A working example, `dice.py`:

```python
#!/usr/bin/env python3
# Display name: Dice
# 显示名称：掷骰子
# Description: Roll dice and return each face and the total.
# 描述：掷骰子，交回每一颗和总数。
# Timeout: 10
# Parameters:
# {"type": "object", "properties": {"count": {"type": "integer"}, "sides": {"type": "integer"}}, "required": ["count", "sides"]}
import json
import random
import sys

args = json.load(sys.stdin)
count = max(1, min(int(args["count"]), 100))
sides = max(2, min(int(args["sides"]), 1000))
faces = [random.randint(1, sides) for _ in range(count)]
print(f"{count}d{sides}: {faces} = {sum(faces)}")
```

On Linux and macOS, `chmod +x dice.py`.

**The header**

| Key | Notes |
|---|---|
| `Description` | The description for the model, short English, required |
| `描述` | The description for people (Chinese) |
| `Display name`, `显示名称` | The name for people (English, Chinese); the timeline shows it |
| `Parameters` | The parameters' JSON Schema, from the next line on, may span lines; none when omitted |
| `Timeout` | Seconds, default 60 |
| `Need load` | Loaded on demand unless you write it: the AI normally sees only the name and one line and loads the full parameters when needed, which saves money. `no`: always send the full parameters; write it for ones used often |
| `Clicall` | `no`: the AI cannot call it with `miyu call` inside a command, only directly. Unset means it can |

**How it runs**

- Started by its `#!` line (on Windows, the interpreter is found by extension: `.py` uses `python`, `.ps1` PowerShell, `.js` `node`).
- The arguments are one JSON object, on stdin and in the environment variable `MIYU_ARGS_JSON`.
- Stdout is the result handed to the AI; a non-zero exit code counts as failure, so write the reason to the output.
- The working directory is the session's workspace; a writable cache directory is in `MIYU_SCRIPT_CACHE_DIR`.
- It runs under the same rules as commands the AI runs: in the sandbox (can only write the workspace, can't read Miyu's data directory, the script file itself is readable); without a sandbox, a person must approve.

## Skills

The public Agent Skills format (Claude Code, Codex and others read it too), so existing skills from the web just work. `skills/<name>/SKILL.md`:

```markdown
---
name: take-notes
description: "Keep meeting notes in notes/YYYY-MM-DD.md: decisions, owners, open questions. Use when the user asks to take or tidy notes, 记笔记."
metadata:
  display_name: Take notes
  summary: Meeting notes in a fixed format
  display_name_zh: 记笔记
  summary_zh: 照固定格式记会议笔记
---

# Taking notes

1. Create `notes/` in the workspace if it doesn't exist.
2. …
```

| Field | Notes |
|---|---|
| `name` | Lowercase letters, digits, `-`; same as the folder name |
| `description` | For the model: English, what it is and when to use it; may include a few trigger words people actually say in other languages. Quote it (an unquoted "colon space" inside won't parse) |
| `metadata` `display_name`, `summary` | Name and one line for people (English); Chinese and Japanese in `display_name_zh`, `summary_zh`, `display_name_ja`, `summary_ja` |

The AI normally sees only the name, the description and the path of `SKILL.md`, and reads the body when it's useful. It can read other files in the folder (references, bundled scripts) but not change them; bundled scripts aren't tools, so the body says how to run them.

## Command templates

Claude Code's format: `commands/<name>.md`. When a person types `/<name> args`, the body is sent to the AI as that person's message.

```markdown
---
description: Review the changes in the working tree
argument-hint: "[focus]"
metadata:
  summary_zh: 检查工作区里的改动
---

Review the uncommitted changes in this repository. Focus on: $ARGUMENTS
```

- `$ARGUMENTS` becomes everything after the name, `$ARGUMENTS[0]` the first word; if the body doesn't use `$ARGUMENTS`, the arguments are appended.
- `description` is the line in the command menu (English); Chinese and Japanese go in `metadata` as `summary_zh`, `summary_ja`. `argument-hint` hints at what follows the name.
- Names: start with a lowercase letter; only lowercase letters, digits, `-`. A template with the same name as a Miyu built-in command doesn't count (the built-in wins).
- The timeline shows the line the person typed. Who can use it is the same as for built-in commands.

For slash commands handled by a program (looking something up, changing state, replying with text), use an extension's `[[slash]]`; see [Writing an extension](extensions.en.md).
