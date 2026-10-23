//! 给 `miyu` 加的子命令（9-2 转交）：一个包可以加几个（施工 F-13，`[[command]]`），旧的一张 `[command]` 照旧读，是只有一个。
//! 几个可以是同一个程序：转交时把 `args` 加在人敲的参数前面（终端界面的包加 `miyu shellhook`，转交成
//! `miyu-tui shellhook …`，项目主人 2026-10-20 定 shellhook 并进终端界面的包）。

use toml_edit::{Item, TableLike};

use super::reader::{Reader, command_name, line_of, program_name};
use super::{Code, Command, Problem};

/// 读 `command` 那一格：一张表是一个，表的数组照写的先后；没写的、不是表的是空的。同一个包里两个同名的报在后一个。
pub(super) fn read(reader: &Reader<'_>, item: Option<&Item>) -> Result<Vec<Command>, Problem> {
    let tables: Vec<&dyn TableLike> = match item {
        Some(Item::ArrayOfTables(array)) => {
            array.iter().map(|table| table as &dyn TableLike).collect()
        }
        Some(item) => item.as_table_like().into_iter().collect(),
        None => Vec::new(),
    };
    let Some(at) = item else {
        return Ok(Vec::new());
    };
    let mut commands: Vec<Command> = Vec::new();
    for table in tables {
        let command = one(reader, table, at)?;
        if commands.iter().any(|seen| seen.name == command.name) {
            return Err(Problem {
                line: command.line,
                code: Code::CommandTaken,
                detail: format!("command.{}", command.name),
                message: format!("command {:?} is written twice", command.name),
            });
        }
        commands.push(command);
    }
    Ok(commands)
}

/// 一个：`name`、`program`、`about` 必写，`args` 可以不写。
fn one(reader: &Reader<'_>, table: &dyn TableLike, at: &Item) -> Result<Command, Problem> {
    reader.only(table, "command", super::fields::COMMAND)?;
    let item = reader.required(table, at, "command", "name")?;
    let line = item.span().map(|span| line_of(reader.text, span.start));
    let name = item
        .as_str()
        .filter(|name| command_name(name))
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadCommandName,
                "command.name",
                "command.name must start with a lowercase letter and use only lowercase letters, digits and -".to_string(),
            )
        })?;
    let item = reader.required(table, at, "command", "program")?;
    let program = item
        .as_str()
        .filter(|program| program_name(program))
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadProgram,
                "command.program",
                "command.program must be a program name without a path".to_string(),
            )
        })?;
    let about = reader.phrases(
        reader.required(table, at, "command", "about")?,
        "command.about",
    )?;
    Ok(Command {
        name: name.to_string(),
        program: program.to_string(),
        args: reader.texts(table, "command", "args")?,
        about,
        line,
    })
}

impl super::Manifest {
    /// 这个包的程序那一个：第一个子命令（施工 F-13）。拉起扩展、头，`miyu check` 照它的程序；没写子命令的没有。
    pub fn command(&self) -> Option<&Command> {
        self.commands.first()
    }

    /// 叫 `name` 的那个子命令。
    pub fn command_named(&self, name: &str) -> Option<&Command> {
        self.commands.iter().find(|command| command.name == name)
    }
}

#[cfg(test)]
mod tests;
