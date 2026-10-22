# Scripts, skills, command templates

None of these need a long-running program; one file is enough. Chinese version: [scripts-skills.md](scripts-skills.md).

| | What it is | How the AI uses it |
|---|---|---|
| **Script** | An executable file; one tool | Called like any other tool |
| **Skill** | A `SKILL.md`; a way of doing something | Normally sees only its name and one line; reads the body when it needs it |
| **Command template** | A `<name>.md`; a slash command | When a person types `/<name>`, the body is sent to the AI as that person's message |

Where: in a package, under the package folder's `scripts/`, `skills/`, `commands/`, with a manifest of just `[package]`; just for you, straight into `~/.miyu/home/admin/scripts/`, `skills/`, `commands/` (same names there win). The next session sees them; each script, skill and template is switched separately in presets. Broken ones aren't registered, and `miyu check` says which and why.

Links work too, pointing at files or folders elsewhere (say, kept together in a repository of your own): what the link points at counts, and the AI sees the real path; broken links are skipped. Links into other parts of Miyu's data folder (`system/`, other accounts) don't count, and `miyu check` says where they point.

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
| `Venues` | Which sessions get it: some of `local` (this machine: the terminal, the web page and so on), `private` (private chats on a messaging platform), `group` (groups), comma separated, e.g. `Venues: local, private`. Unset means every kind of session; if it is wrong the whole script is not registered and `miyu check` reports it |

The header is the run of comment lines at the very top of the file; it ends at the first line that is not a comment. Leave a blank line after the header before any other comment, or a sentence like "Timeout: …" in your notes is read as part of the header.

**How it runs**

- Started by its `#!` line (on Windows, the interpreter is found by extension: `.py` uses `python`, `.ps1` PowerShell, `.js` `node`).
- The arguments are one JSON object. Stdin always has all of it; the environment variable `MIYU_ARGS_JSON` also has it when it is at most 30 KiB. Long arguments are only on stdin, so reading stdin is the safe way.
- Stdout is the result handed to the AI; a non-zero exit code counts as failure, so write the reason to the output.
- The working directory is the session's workspace; a writable cache directory is in `MIYU_SCRIPT_CACHE_DIR` (under the system temp folder, gone after a reboot or a temp cleanup).
- Anything that must last (a login, where the last run stopped) goes in `MIYU_SCRIPT_STATE_DIR`: one folder per account and script, still there next time. Group chats and strangers' private chats run under a different account from your own sessions, so each gets its own folder. For a script in a package, it is deleted when the package is removed; for one you put in `scripts/` yourself, it stays after you delete the script, and `miyu check` points it out.
- Who called: `MIYU_CALLER_VENUE` is the kind of session it was called in (`local` on this machine, the venue id for a group or private chat, e.g. `qq:group:123`); `MIYU_CALLER_OWNER` says whether the person speaking this turn is the machine's owner (`1` or `0`; `0` when one of several people in a group is not the owner, or when it can't be told).
- These two are facts, not a gate: they can't stop the AI from writing `MIYU_CALLER_OWNER=1 ./script` in a command itself. To keep a script to some kinds of session, write `Venues` in the header; use the variables only to adjust what the script does, for example read-only in groups:

  ```python
  import os
  if os.environ.get("MIYU_CALLER_OWNER") != "1":
      print("Read-only here: only the owner can change the title.")
  ```

- It runs under the same rules as commands the AI runs: in the sandbox (can only write the workspace, can't read Miyu's data directory, the script file itself is readable); without a sandbox, a person must approve.

**Handing over images**

To let the AI see images directly (screenshots, frames from a video), copy them into `MIYU_SCRIPT_IMAGES_DIR`: a new empty folder for each call, which Miyu collects after the run and then deletes.

```python
import os, shutil
shutil.copy("frame-01.png", os.environ["MIYU_SCRIPT_IMAGES_DIR"])
print("Grabbed 1 frame.")
```

- Taken in file-name order, at most 4; PNG, JPEG, GIF or WebP, each at most 5 MiB and at most 8000 pixels per side.
- Only regular files directly in the folder count, and links are not followed: copy the images in, don't link them.
- Anything that doesn't fit is not handed over and the AI doesn't learn of it; the run log has a `script image dropped` line saying which file and why.
- A call stopped at its timeout hands over no images; a non-zero exit code still does.

**Scripts that must run outside the sandbox**

Scripts that touch hardware or need root (`sudo`) don't work in the sandbox. A script can't ask for this itself, whatever its header says: the admin grants it in the system config `system/config.toml`, and the value is where the tool comes from.

```toml
[sandbox.tools]
ceiling-light = "~home"   # a script you put in scripts/
lights = "smart-home"     # from a package: the package's id
```

- A granted tool runs outside the sandbox without asking in the admin's own sessions (terminal, web, the owner's QQ private chat, desktop voice). Group chats and strangers' private chats still refuse it, and so does read-only; dangerous commands still ask.
- A grant whose source doesn't match doesn't count: delete the script, or let another package bring a tool with the same name, and the grant lapses. `miyu check` warns about each grant whose tool is gone or whose source doesn't match, printing the current source for the latter, and also flags a grant for `shell`.
- A granted tool has all of your own permissions: only grant scripts whose code you have read and whose arguments are safe.
- A grant only counts for a real file kept in `scripts/` in your home folder (or installed as a package): if the file is a link, if `scripts/` itself is a link, or if the file sits somewhere the AI can write this time (such as the workspace), it isn't granted and still runs in the sandbox (the AI could change those). `miyu check` reminds you and prints where the file really is.

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

- `$ARGUMENTS` becomes everything after the name, `$ARGUMENTS[0]` the first word (split on whitespace; a value with spaces goes in English double quotes, so for `/review "my file.rs"` `$ARGUMENTS[0]` is `my file.rs`); if the body doesn't use `$ARGUMENTS`, the arguments are appended.
- `description` is the line in the command menu (English); Chinese and Japanese go in `metadata` as `summary_zh`, `summary_ja`. `argument-hint` hints at what follows the name.
- Names: start with a lowercase letter; only lowercase letters, digits, `-`. A template with the same name as a Miyu built-in command doesn't count (the built-in wins).
- The timeline shows the line the person typed. Who can use it is the same as for built-in commands.

For slash commands handled by a program (looking something up, changing state, replying with text), use an extension's `[[slash]]`; see [Writing an extension](extensions.en.md).
