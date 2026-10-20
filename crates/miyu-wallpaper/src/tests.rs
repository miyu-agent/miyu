//! 读各家的回答（照各家真的输出剪的样本）、认图、换没换的记号。

use std::path::PathBuf;

use crate::parse::{
    fehbg, gsettings_uri, hyprpaper_active, is_heic, kde_image, kde_package_pick, macos_index,
    nitrogen_cfg, osascript_path, path_or_uri, reg_wallpaper, sniff, swaybg_image, swww_query,
    waypaper_ini, wpaperd_all, xfce_last_image,
};
use crate::{Wallpaper, image_kind};

#[test]
fn awww_swww_take_the_first_screen_showing_an_image() {
    // 2026-10-11 本机 niri + awww 0.11 的输出：一个屏幕一行
    let out = ": eDP-1: 1707x960, scale: 1.5, currently displaying: image: /home/u/Pictures/Wallpapers/a b.jpg\n\
               : DP-2: 2560x1440, scale: 1, currently displaying: image: /home/u/Pictures/Wallpapers/c.png\n";
    assert_eq!(
        swww_query(out),
        Some(PathBuf::from("/home/u/Pictures/Wallpapers/a b.jpg"))
    );
    // 老版本行首没有 `: `；第一个屏幕显示纯色的跳过
    let out = "eDP-1: 1920x1080, scale: 1, currently displaying: color: 000000\nHDMI-A-1: 1920x1080, scale: 1, currently displaying: image: /w/x.webp\n";
    assert_eq!(swww_query(out), Some(PathBuf::from("/w/x.webp")));
    assert_eq!(swww_query(""), None);
}

#[test]
fn hyprpaper_takes_the_first_monitor() {
    assert_eq!(
        hyprpaper_active("eDP-1 = /home/u/w.png\nDP-1 = /home/u/v.png\n"),
        Some(PathBuf::from("/home/u/w.png"))
    );
    assert_eq!(hyprpaper_active("no wallpapers active\n"), None);
}

#[test]
fn gnome_uri_is_unquoted_and_unescaped() {
    assert_eq!(
        gsettings_uri("'file:///home/u/Pictures/my%20wall.jpg'\n"),
        Some(PathBuf::from("/home/u/Pictures/my wall.jpg"))
    );
    assert_eq!(
        gsettings_uri("'file:///home/u/%E5%A3%81%E7%BA%B8.png'\n"),
        Some(PathBuf::from("/home/u/壁纸.png"))
    );
    // 没设的、坏的转义、不是本机文件的
    assert_eq!(gsettings_uri("''\n"), None);
    assert_eq!(gsettings_uri("'file:///a%zz.png'"), None);
    assert_eq!(gsettings_uri("'https://example.invalid/a.png'"), None);
    assert_eq!(
        path_or_uri("/plain/path.jpg"),
        Some(PathBuf::from("/plain/path.jpg"))
    );
}

#[test]
fn kde_reads_the_image_of_the_first_wallpaper_section() {
    let config = "[ActionPlugins][0]\nRightButton;NoModifier=org.kde.contextmenu\n\n\
                  [Containments][1][Wallpaper][org.kde.color][General]\nColor=0,0,0\n\n\
                  [Containments][1][Wallpaper][org.kde.image][General]\nImage=file:///home/u/Pictures/k%20de.png\nSlidePaths=/usr/share/wallpapers/\n\n\
                  [Containments][2][Wallpaper][org.kde.image][General]\nImage=/usr/share/wallpapers/Next/\n";
    assert_eq!(
        kde_image(config),
        Some(PathBuf::from("/home/u/Pictures/k de.png"))
    );
    assert_eq!(
        kde_image(
            "[Containments][2][Wallpaper][org.kde.image][General]\nImage=/usr/share/wallpapers/Next/\n"
        ),
        Some(PathBuf::from("/usr/share/wallpapers/Next/"))
    );
    // 别的节里的 Image= 不算
    assert_eq!(
        kde_image("[Containments][1][General]\nImage=/x.png\n"),
        None
    );
}

#[test]
fn kde_package_picks_the_largest_resolution() {
    let names = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        kde_package_pick(&names(&["1920x1080.png", "5120x2880.png", "3840x2160.png"]))
            .map(String::as_str),
        Some("5120x2880.png")
    );
    assert_eq!(
        kde_package_pick(&names(&["screenshot.jpg", "base.png"])).map(String::as_str),
        Some("base.png")
    );
    assert_eq!(kde_package_pick(&[]), None);
}

#[test]
fn macos_and_windows_answers() {
    assert_eq!(
        osascript_path("/System/Library/Desktop Pictures/Sonoma.heic\n"),
        Some(PathBuf::from(
            "/System/Library/Desktop Pictures/Sonoma.heic"
        ))
    );
    assert_eq!(
        osascript_path("execution error: Not authorized (-1743)"),
        None
    );
    let out = "\r\nHKEY_CURRENT_USER\\Control Panel\\Desktop\r\n    WallPaper    REG_SZ    C:\\Users\\u\\Pictures\\wall.jpg\r\n\r\n";
    assert_eq!(
        reg_wallpaper(out),
        Some(PathBuf::from(r"C:\Users\u\Pictures\wall.jpg"))
    );
    assert_eq!(reg_wallpaper("    WallPaper    REG_SZ    \r\n"), None);
}

#[test]
fn only_images_the_browser_can_decode() {
    assert_eq!(sniff(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"), Some("image/png"));
    assert_eq!(
        sniff(&[0xff, 0xd8, 0xff, 0xe0, 0, 0x10]),
        Some("image/jpeg")
    );
    assert_eq!(sniff(b"RIFF\x10\0\0\0WEBPVP8 "), Some("image/webp"));
    assert_eq!(sniff(b"\0\0\0\x1cftypavif\0\0"), Some("image/avif"));
    // macOS 的动态壁纸是 HEIC，浏览器解不了
    assert_eq!(sniff(b"\0\0\0\x18ftypheic\0\0"), None);
    assert_eq!(sniff(b"\0\0\0\x18ftypmp42\0\0"), None);
    assert_eq!(sniff(b""), None);
}

#[test]
fn signature_changes_when_the_file_changes() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!("miyu-wallpaper-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("w.png");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\n first")?;
    assert_eq!(image_kind(&path)?, Some(("image/png", false)));
    assert_eq!(image_kind(&dir)?, None, "目录不是图");
    let w = Wallpaper {
        path: path.clone(),
        from: "test",
        mime: "image/png",
        heic: false,
        watch: Vec::new(),
    };
    let before = w.signature()?;
    assert_eq!(w.signature()?, before, "没动过的一样");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\n second, longer")?;
    assert_ne!(w.signature()?, before, "换了内容（大小变了）的不一样");
    assert_eq!(w.read()?.len(), 23);
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn more_desktops_and_wallpaper_programs() {
    let xml = r#"<channel name="xfce4-desktop" version="1.0"><property name="backdrop" type="empty"><property name="screen0" type="empty">
      <property name="monitoreDP-1" type="empty"><property name="workspace0" type="empty">
        <property name="color-style" type="int" value="0"/><property name="last-image" type="string" value="/home/u/Pictures/a&amp;b.jpg"/>
      </property></property></property></property></channel>"#;
    assert_eq!(
        xfce_last_image(xml),
        Some(PathBuf::from("/home/u/Pictures/a&b.jpg"))
    );
    let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        swaybg_image(&args(&[
            "swaybg", "-o", "eDP-1", "-i", "/w/a.png", "-m", "fill"
        ])),
        Some(PathBuf::from("/w/a.png"))
    );
    assert_eq!(
        swaybg_image(&args(&["swaybg", "--image=/w/b.png"])),
        Some(PathBuf::from("/w/b.png"))
    );
    assert_eq!(swaybg_image(&args(&["swaybg", "-c", "#000000"])), None);
    assert_eq!(
        wpaperd_all("eDP-1: /home/u/w/1.jpg\nHDMI-A-1: /home/u/w/2.jpg\n"),
        Some(PathBuf::from("/home/u/w/1.jpg"))
    );
    let ini = "[Settings]\nlanguage = en\nfolder = ~/Pictures\nwallpaper = ~/Pictures/w1.png,~/Pictures/w2.png\nbackend = swww\n";
    assert_eq!(
        waypaper_ini(ini, "/home/u"),
        Some(PathBuf::from("/home/u/Pictures/w1.png"))
    );
    assert_eq!(
        waypaper_ini("[Other]\nwallpaper = /x.png\n", "/home/u"),
        None,
        "别的节里的不算"
    );
    assert_eq!(
        fehbg("#!/bin/sh\nfeh --no-fehbg --bg-fill '/home/u/it'\\''s.jpg' \n"),
        Some(PathBuf::from("/home/u/it's.jpg"))
    );
    assert_eq!(
        nitrogen_cfg("[xin_-1]\nfile=/home/u/n.jpg\nmode=5\nbgcolor=#000000\n"),
        Some(PathBuf::from("/home/u/n.jpg"))
    );
}

#[test]
fn macos_index_plist_and_heic() {
    // `plutil -convert xml1` 转出来的样子（剪过）：第一个 file:// 是目录（壁纸文件夹）的不算
    let xml = "<dict><key>Provider</key><string>com.apple.wallpaper.choice.image</string>\n\
               <key>relative</key><string>file:///Users/u/Pictures/</string>\n\
               <key>relative</key><string>file:///Users/u/Pictures/My%20Wall.heic</string></dict>";
    assert_eq!(
        macos_index(xml),
        Some(PathBuf::from("/Users/u/Pictures/My Wall.heic"))
    );
    assert_eq!(
        macos_index(
            "<dict><key>Provider</key><string>com.apple.wallpaper.choice.aerials</string></dict>"
        ),
        None
    );
    assert!(is_heic(b"\0\0\0\x18ftypheic\0\0"));
    assert!(is_heic(b"\0\0\0\x18ftypmif1\0\0"));
    assert!(!is_heic(b"\0\0\0\x1cftypavif\0\0"));
}

#[test]
fn matugen_output_lives_in_the_cache_dir() {
    let path = crate::matugen::output_path();
    assert!(
        path.as_ref()
            .is_some_and(|p| p.ends_with("miyu/matugen.json") || p.ends_with(r"miyu\matugen.json")),
        "{path:?}"
    );
}

/// 测盯文件用的目录：照真实路径（macOS 的临时目录在链接 `/var` 下面，系统的通知报的是真实路径）。
fn watch_root(name: &str) -> std::io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!("miyu-wallpaper-{name}-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    std::fs::create_dir_all(&root)?;
    root.canonicalize()
}

#[test]
fn watch_plan_watches_files_themselves_and_skips_missing_ones() -> std::io::Result<()> {
    use notify::RecursiveMode::{NonRecursive, Recursive};
    let root = watch_root("plan")?;
    std::fs::create_dir_all(root.join("cache"))?;
    std::fs::write(root.join("a.ini"), "x")?;
    let targets = [
        root.join("a.ini"),
        root.join("missing/deeper/b.ini"),
        root.join("cache"),
    ];
    let plan = crate::watch::plan(&targets);
    assert_eq!(
        plan.get(&root.join("a.ini")),
        Some(&NonRecursive),
        "在的文件只盯它自己"
    );
    assert_eq!(
        plan.get(&root.join("cache")),
        Some(&Recursive),
        "目录整个往下盯"
    );
    assert_eq!(plan.len(), 2, "还没有的不盯，也不盯上层：{plan:?}");
    assert!(
        crate::watch::touches(&targets, &root.join("cache/0.12/DP-2")),
        "目录里面的"
    );
    assert!(
        !crate::watch::touches(&targets, &root.join("other.txt")),
        "同一层别的文件不算"
    );
    assert_eq!(
        crate::watch::resolve(&root.join("missing/deeper/b.ini")),
        root.join("missing/deeper/b.ini")
    );
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn watch_list_is_matugen_and_files_of_only_the_source_found() {
    assert_eq!(
        crate::watch::watch_list(),
        crate::matugen::output_path()
            .into_iter()
            .collect::<Vec<_>>(),
        "总要盯的只有 matugen 那一份"
    );
    if cfg!(all(unix, not(target_os = "macos"))) && std::env::var_os("HOME").is_some() {
        let awww = crate::watch::files_of("awww");
        assert!(awww.len() == 1 && awww[0].ends_with("awww"), "{awww:?}");
        assert!(crate::watch::files_of("feh")[0].ends_with(".fehbg"));
        assert!(crate::watch::files_of("swaybg").is_empty(), "不落文件的");
    }
}

#[test]
fn watch_calls_back_when_listed_files_change_and_only_then() -> std::io::Result<()> {
    use std::sync::mpsc;
    use std::time::Duration;
    let root = watch_root("live")?;
    let file = root.join("wall.ini");
    let later = root.join("later.ini");
    let cache = root.join("cache");
    std::fs::write(&file, "first")?;
    std::fs::create_dir_all(&cache)?;
    let (tx, rx) = mpsc::channel();
    let watch = crate::watch::Watch::start(
        vec![file.clone(), later.clone(), cache.clone()],
        move || {
            let _sent = tx.send(());
        },
    )?;
    let quiet = |what: &str| {
        assert!(
            rx.recv_timeout(Duration::from_millis(900)).is_err(),
            "{what}：不该叫"
        );
    };
    let called = |what: &str| {
        assert!(
            rx.recv_timeout(Duration::from_secs(10)).is_ok(),
            "{what}：该叫"
        );
        // 一阵收齐叫一次，后面还在路上的收掉
        std::thread::sleep(Duration::from_millis(700));
        while rx.try_recv().is_ok() {}
    };
    std::fs::write(root.join("other.txt"), "x")?;
    quiet("同一层别的文件");
    std::fs::write(&later, "x")?;
    quiet("还没有的建出来了（不盯，靠拿回焦点再找）");
    std::fs::write(&file, "second")?;
    called("改了文件");
    assert_eq!(std::fs::read_to_string(&file)?, "second");
    quiet("只是读");
    std::fs::write(root.join("wall.ini.tmp"), "third")?;
    std::fs::rename(root.join("wall.ini.tmp"), &file)?;
    called("换成了别的文件");
    std::fs::write(&file, "fourth")?;
    called("换过以后再改（重盯了新的那一个）");
    std::fs::create_dir_all(cache.join("0.12"))?;
    std::fs::write(cache.join("0.12/DP-2"), "x")?;
    called("缓存目录里面建出了文件");
    std::fs::write(cache.join("0.12/DP-2"), "y")?;
    called("缓存目录里面的文件变了");
    watch.set(Vec::new())?;
    std::fs::write(&file, "fifth")?;
    quiet("清单空了");
    drop(watch);
    std::fs::remove_dir_all(&root)?;
    Ok(())
}
