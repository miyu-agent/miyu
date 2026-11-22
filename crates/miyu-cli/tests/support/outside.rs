//! 数据根外面的临时目录。

use std::path::{Path, PathBuf};

use super::next;

/// 数据根外面的一个临时目录，用完就删：当项目目录，或者当工作区外面。放在 cargo 给集成测试的 `target/tmp`
/// 下面：系统的临时目录整个能读能写，放在里面就造不出「工作区外面」（施工 4-3 下）。
pub struct Outside(pub PathBuf);

impl Outside {
    pub fn new() -> Outside {
        std::fs::create_dir_all(env!("CARGO_TARGET_TMPDIR")).expect("建得了目录");
        let dir = fenced(Path::new(env!("CARGO_TARGET_TMPDIR"))).join(format!(
            "miyu-cli-out-{}-{}",
            std::process::id(),
            next()
        ));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        Outside(dir)
    }

    /// 在里面写一份文件，交回它的路径。
    pub fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).expect("写得进");
        path
    }

    /// 这个目录，写成字。
    pub fn text(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

/// 在 cargo 给集成测试的 `target/tmp` 根上放一个指向不存在处的 `.git` 文件（施工 FX-25）。`target` 多半在仓库里面，测试在这
/// 底下跑 git，git 往上找会找到真的仓库：LiteMiyu 的一条测试放行 `git push --force`，十天里每跑一次整套测试就真的强推一次。
/// 有了它，这底下的 git 一律报「不是仓库」（退出码 128），往上找项目配置也停在这里。
fn fenced(dir: &Path) -> &Path {
    let made = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(".git"))
        .and_then(|mut file| std::io::Write::write_all(&mut file, b"gitdir: miyu-no-repository\n"));
    match made {
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => {
            panic!("在 {} 放不进 .git：{error}", dir.display())
        }
        _ => dir,
    }
}

impl Drop for Outside {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
