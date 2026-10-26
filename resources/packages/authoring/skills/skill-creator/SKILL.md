---
name: skill-creator
description: "Write a new Miyu skill or change one. Use when asked to make, write or improve a skill (写技能、做个技能)."
metadata:
  display_name: Write a skill
  display_name_zh: 写技能
  summary: Write a skill as a package, check it, install it
  summary_zh: 照包的样子写一个技能，检查后装上
---

# Write a skill

A skill is a folder with a `SKILL.md`: a short head that says when to use it, then a body that teaches one kind of task. You only see the name and description until you read the body, so the description decides when the skill gets used.

## Where it goes

Skills are installed as packages. Write the package in the workspace, then install it:

```text
<package-id>/
├── package.toml
└── skills/
    └── <skill-name>/
        └── SKILL.md      # plus any reference files the body points to
```

`package.toml` for a package that only carries skills and scripts has one table:

```toml
[package]
name = { en = "Notes", zh = "笔记" }
```

The folder name is the package id: lowercase letters, digits, `-`, `_`, starting with a letter. Do not write into the Miyu data folder: it is closed to file tools.

## The head

```yaml
---
name: take-notes
description: "Keep tidy notes in notes/. Use when asked to note, log or record something (记一下、做笔记)."
metadata:
  display_name: Take notes
  display_name_zh: 记笔记
  summary: Keeps notes in one place
  summary_zh: 把笔记记在一个地方
---
```

- `name`: 1 to 64 lowercase letters, digits and single `-`, the same as the folder.
- `description`: English, what it does and when to use it, up to 1024 characters. A few words people really say may be in Chinese. Put it in double quotes: a bare `: ` inside breaks the YAML and the skill is skipped.
- `metadata`: names and a one-line summary for people, `_zh` and `_ja` for other languages. Optional.

## The body

- One task per skill. Steps in order, short sentences, the commands or file formats to use.
- Put long references in separate files next to `SKILL.md` and say when to read them.
- Do not repeat what every task needs; write what is special to this one.

## Check and install

1. `miyu pkg info <folder>` in a shell command: it says whether it installs and lists the skills; anything under `skipped` will not load, fix it first.
2. `miyu pkg install <folder>` the same way. The person is asked before it is installed. The skill shows up from the next turn.
3. To change it later, edit the files in the workspace and install the folder again: the same id replaces the old one.
