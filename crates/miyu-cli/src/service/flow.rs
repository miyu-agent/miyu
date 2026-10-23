//! 装、卸、看怎么走（施工 Z-1 下）：照 `plan.rs` 算好的，经 [`Machine`] 写文件、跑命令，交回要说的那一句。

use super::machine::Machine;
use super::plan::{self, Manager, Place};
use super::{Said, ServiceAction};

/// 走一次 `action`。
pub(crate) fn run(machine: &dyn Machine, action: ServiceAction) -> Said {
    let Some(place) = machine.place() else {
        return Said::NoHome;
    };
    let manager = machine.manager();
    match action {
        ServiceAction::Enable => enable(machine, manager, &place),
        ServiceAction::Disable => disable(machine, manager, &place),
        ServiceAction::Status => status(machine, manager, &place),
    }
}

/// 写好、交给服务管理器。已经装过的照现在的 `miyu` 位置、数据根重写一遍（换了位置的跟着换）。
fn enable(machine: &dyn Machine, manager: Manager, place: &Place) -> Said {
    let exe = match machine.exe() {
        Ok(exe) => exe,
        Err(error) => return Said::Failed("current_exe".to_string(), error.to_string()),
    };
    let data_root = machine.data_root();
    let unit = match plan::unit(manager, &exe, data_root.as_deref(), place) {
        Ok(unit) => unit,
        Err(problem) => return Said::Cannot(problem),
    };
    if let Err(error) = machine.write(&unit.file, &unit.text) {
        return Said::Failed(unit.file.display().to_string(), error.to_string());
    }
    // launchd 再登记一次会报已经有了：先拿掉旧的，没有的不算错。
    if manager == Manager::Launchd {
        for command in plan::disable(manager, place) {
            machine.run(&command);
        }
    }
    for command in plan::enable(manager, &unit, place) {
        let ran = machine.run(&command);
        if !ran.ok {
            return Said::Failed(command.join(" "), ran.said);
        }
    }
    Said::Enabled(manager, unit.file)
}

/// 停掉、拿掉、删文件。没装的说没装。
fn disable(machine: &dyn Machine, manager: Manager, place: &Place) -> Said {
    let file = match file_of(machine, manager, place) {
        Ok(file) => file,
        Err(said) => return said,
    };
    if !machine.exists(&file) {
        return Said::NotEnabled(manager, file);
    }
    for command in plan::disable(manager, place) {
        let ran = machine.run(&command);
        if !ran.ok {
            return Said::Failed(command.join(" "), ran.said);
        }
    }
    match machine.remove(&file) {
        Ok(()) => Said::Disabled(manager, file),
        Err(error) => Said::Failed(file.display().to_string(), error.to_string()),
    }
}

/// 装没装、开没开。
fn status(machine: &dyn Machine, manager: Manager, place: &Place) -> Said {
    let file = match file_of(machine, manager, place) {
        Ok(file) => file,
        Err(said) => return said,
    };
    let installed = machine.exists(&file);
    let on = match installed {
        true => plan::running(manager, place).map(|command| machine.run(&command).ok),
        false => None,
    };
    Said::Status {
        manager,
        file,
        installed,
        on,
    }
}

/// 写的那一个文件在哪：照现在的位置算（数据根不影响位置）。不知道 `miyu` 在哪的说没成。
fn file_of(
    machine: &dyn Machine,
    manager: Manager,
    place: &Place,
) -> Result<std::path::PathBuf, Said> {
    let exe = machine
        .exe()
        .map_err(|error| Said::Failed("current_exe".to_string(), error.to_string()))?;
    plan::unit(manager, &exe, None, place)
        .map(|unit| unit.file)
        .map_err(Said::Cannot)
}
