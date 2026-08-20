use super::apply::apply_all;
use super::restore::restore_all;
use super::{check, new_progress, LibStatus, INJECT_MARK, UNLOCK_MARK};
use crate::core::{crypto, locate, modules, slots};

/// 序列化使用进程级环境变量（EDITOR_PATCH_BACKUP_DIR）的端到端测试，防并发互踩
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_slots_embedded() {
    // 编译期内嵌的 slots 文件可用（当前版本 199/169 必须存在）
    assert!(slots::has_slots("script", 199));
    assert!(slots::has_slots("xdeditor", 160));
    assert!(slots::has_slots("xdeditor", 169));
    assert!(!slots::has_slots("script", 999));
    // 复用判定的版本枚举：xdeditor 低于 169 的最近版本是 160
    let below = slots::versions_below("xdeditor", 169);
    assert!(below.first() == Some(&160), "{below:?}");
}

/// 三级回退第 3 级：无精确插槽（也无 manifest 可复用）时运行时注入
#[test]
fn test_runtime_inject_fallback() {
    let _guard = ENV_LOCK.lock().unwrap();
    let base = std::env::temp_dir().join(format!("editor_patch_inject_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let backup_dir = base.join("backup");
    std::env::set_var("EDITOR_PATCH_BACKUP_DIR", &backup_dir);

    // 项目结构（script 包指向无插槽的 v999）
    let project = base.join("project_x");
    std::fs::create_dir_all(project.join("project")).unwrap();
    std::fs::create_dir_all(project.join("script")).unwrap();
    std::fs::write(
        project.join("project").join("map_settings.json"),
        r#"{"api_version": {"api_version": 13}}"#,
    )
    .unwrap();
    let editor_root = base.join("Update").join("editor-pd.spark.xd.com");
    std::fs::write(
        project.join("script").join("tsconfig.json"),
        format!(
            r#"{{"compilerOptions": {{"typeRoots": ["{}"]}}}}"#,
            editor_root.display().to_string().replace('\\', "/")
                + "/Res/_m/maps/global_default/53/global_default/script/"
        ),
    )
    .unwrap();
    std::fs::create_dir_all(&editor_root).unwrap();
    // 引擎运行根（editor_root 上两级）需含 version-<api> 目录（engine_root 回退校验）
    std::fs::create_dir_all(base.join("version-13")).unwrap();
    std::fs::write(
        editor_root.join("api_pak_version.json"),
        r##"{"#package_path": {"script": "Res/_m/script"},
            "13": {"script": 999}}"##,
    )
    .unwrap();

    // script 包 v999：加密入口（含顶层 return）+ 加密 isolation（含禁用行）
    let common = editor_root.join("Res/_m/script/999/script/common");
    std::fs::create_dir_all(&common).unwrap();
    let init = common.join("init.lua");
    std::fs::write(
        &init,
        crypto::encrypt(b"local M = {}\nfunction M.x() end\n\nreturn M\n"),
    )
    .unwrap();
    let iso = common.join("isolation.lua");
    std::fs::write(&iso, crypto::encrypt(b"os.exit = nil\nio.open = nil\n")).unwrap();

    // 应用：v999 无插槽 → 运行时注入路径
    let progress = new_progress();
    let msg = apply_all(&project, &progress).unwrap();
    assert!(msg.contains("运行时注入插槽"), "{msg}");

    // 注入结果校验：入口含插槽标记且 return 保留、isolation 含解锁标记，均为明文
    let init_text = std::fs::read_to_string(&init).unwrap();
    assert!(init_text.contains(INJECT_MARK));
    assert!(init_text.contains("return M"));
    let iso_text = std::fs::read_to_string(&iso).unwrap();
    assert!(iso_text.contains(UNLOCK_MARK));

    // 状态检查：已应用
    let target = locate::locate(&project).unwrap();
    let statuses = check(&target);
    let s = statuses.iter().find(|s| s.pkg == "script").unwrap();
    assert_eq!(s.status, LibStatus::Applied);

    // 还原：回到加密原始字节
    let progress2 = new_progress();
    restore_all(&project, &progress2).unwrap();
    let raw = std::fs::read(&init).unwrap();
    assert!(raw.starts_with(b"TNND"));

    let _ = std::fs::remove_dir_all(&base);
}

/// 端到端：临时目录双库（加密+明文混合）整库流程
#[test]
fn test_apply_restore_flow() {
    let _guard = ENV_LOCK.lock().unwrap();
    let base = std::env::temp_dir().join(format!("editor_patch_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let backup_dir = base.join("backup");
    std::env::set_var("EDITOR_PATCH_BACKUP_DIR", &backup_dir);

    // 项目结构
    let project = base.join("project_x");
    std::fs::create_dir_all(project.join("project")).unwrap();
    std::fs::create_dir_all(project.join("script")).unwrap();
    std::fs::write(
        project.join("project").join("map_settings.json"),
        r#"{"api_version": {"api_version": 13}}"#,
    )
    .unwrap();
    let editor_root = base.join("Update").join("editor-pd.spark.xd.com");
    std::fs::write(
        project.join("script").join("tsconfig.json"),
        format!(
            r#"{{"compilerOptions": {{"typeRoots": ["{}"]}}}}"#,
            editor_root.display().to_string().replace('\\', "/")
                + "/Res/_m/maps/global_default/53/global_default/script/"
        ),
    )
    .unwrap();
    std::fs::create_dir_all(&editor_root).unwrap();
    // 引擎运行根（editor_root 上两级）需含 version-<api> 目录（engine_root 回退校验）
    std::fs::create_dir_all(base.join("version-13")).unwrap();
    std::fs::write(
        editor_root.join("api_pak_version.json"),
        r##"{"#package_path": {"script": "Res/_m/script", "xdeditor": "Res/_m/xdeditor"},
            "13": {"script": 199, "xdeditor": 160}}"##,
    )
    .unwrap();

    // script 包：加密 init.lua / isolation.lua + 一个明文文件
    // 注意：测试里 api_pak 指 v199，而 slots/script/199/ 内嵌文件会在应用时覆盖 init/isolation
    let common = editor_root.join("Res/_m/script/199/script/common");
    std::fs::create_dir_all(&common).unwrap();
    let iso_original = "-- original isolation\n";
    let iso = common.join("isolation.lua");
    std::fs::write(&iso, crypto::encrypt(iso_original.as_bytes())).unwrap();
    let iso_original_bytes = std::fs::read(&iso).unwrap();
    let init = common.join("init.lua");
    std::fs::write(&init, crypto::encrypt(b"-- original init\n")).unwrap();
    let init_original_bytes = std::fs::read(&init).unwrap();
    let plain_file = common.join("plain_note.lua");
    std::fs::write(&plain_file, "-- 本来就是明文\n").unwrap();

    // xdeditor 包：加密 main.lua
    let xd = editor_root.join("Res/_m/xdeditor/160/xdeditor");
    std::fs::create_dir_all(&xd).unwrap();
    let xd_main = xd.join("main.lua");
    let xd_main_original_bytes = crypto::encrypt(b"-- original xdeditor main\n");
    std::fs::write(&xd_main, &xd_main_original_bytes).unwrap();

    // 应用
    let progress = new_progress();
    let msg = apply_all(&project, &progress).unwrap();
    assert!(msg.contains("插槽文件 2 个"), "{msg}"); // script: init+isolation；xdeditor: main

    // 状态：两库均已应用（插槽文件已覆盖入口/isolation）
    let target = locate::locate(&project).unwrap();
    let statuses = check(&target);
    for s in &statuses {
        assert_eq!(s.status, LibStatus::Applied, "[{}] status={:?} path={}", s.pkg, s.status, s.path);
    }

    // 插槽文件已生效（入口含插槽标记、isolation 含解锁标记），且为明文
    let init_text = std::fs::read_to_string(&init).unwrap();
    assert!(init_text.contains(INJECT_MARK));
    let iso_text = std::fs::read_to_string(&iso).unwrap();
    assert!(iso_text.contains(UNLOCK_MARK));
    // 明文文件未被破坏
    assert_eq!(std::fs::read_to_string(&plain_file).unwrap(), "-- 本来就是明文\n");

    // 默认模块已启用（xdeditor menu_bgd 明文写入）
    let menu_module = modules::patch_dir(&xd).join("menu_bgd").join("main.lua");
    assert!(menu_module.is_file());
    assert!(std::fs::read_to_string(&menu_module).unwrap().contains("window_title_bar_register"));

    // 再应用：幂等（备份不重复、模块保留）
    let progress2 = new_progress();
    let msg2 = apply_all(&project, &progress2).unwrap();
    assert!(msg2.contains("沿用已有备份"), "{msg2}");
    assert!(msg2.contains("保留已启用模块"), "{msg2}");

    // 模拟编辑器升级覆盖：入口换成全新加密原始文件 → 检测为未应用
    std::fs::write(&init, crypto::encrypt(b"-- original init\n")).unwrap();
    let target = locate::locate(&project).unwrap();
    let statuses = check(&target);
    let script_status = statuses.iter().find(|s| s.pkg == "script").unwrap();
    assert_eq!(script_status.status, LibStatus::NotApplied, "覆盖后应检测为未应用");

    // 还原：整库字节级还原（含被插槽覆盖的文件回到加密原始字节）
    let progress3 = new_progress();
    restore_all(&project, &progress3).unwrap();
    assert_eq!(std::fs::read(&iso).unwrap(), iso_original_bytes);
    assert_eq!(std::fs::read(&init).unwrap(), init_original_bytes);
    assert_eq!(std::fs::read(&xd_main).unwrap(), xd_main_original_bytes);
    assert!(!modules::patch_dir(&xd).exists());
    assert!(!modules::patch_dir(&common).exists());

    let _ = std::fs::remove_dir_all(&base);
}
