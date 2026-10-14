use std::fs;
use std::path::Path;
use std::process::Command;

/// # 该函数将配置文件从源位置复制到目标目录中
///
/// 主要用于构建过程中确保配置文件被正确地包含在输出目录里
///
/// ## Panics
/// - 当环境变量"OUT_DIR"不存在时会panic
/// - 当路径操作失败时会panic
/// - 当文件复制失败时会panic
fn main() {
    // println!("cargo:warning=🔍 正在运行 build.rs");
    // 获取项目根目录
    let project_root = env!("CARGO_MANIFEST_DIR");
    // 获取编译脚本文件的输出目录路径
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // 构造目标文件路径，通过向上回溯OUT_DIR的父级目录来定位
    let dest_dir_path = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("Failed to get parent directory");
    // println!(
    //     "cargo:warning=PROJECT_ROOT: {project_root:?} OUT_DIR: {out_dir} dest_path: {dest_dir_path:?}"
    // );

    setup_web_frontend(project_root);

    const CONFIG_EXTS: &[&str] = &["toml", "json", "json5", "yml", "yaml", "ini", "ron"];

    let pkg_name = env!("CARGO_PKG_NAME");

    // 复制应用及其环境配置文件到输出目录
    for suffix in ["", "-dev", "-test", "-prod"] {
        let file_name = format!("{pkg_name}{suffix}");
        for ext in CONFIG_EXTS {
            copy_file(project_root, &file_name, ext, dest_dir_path);
        }
    }

    // 复制日志的配置文件到输出目录
    for ext in CONFIG_EXTS {
        copy_file(project_root, "log", ext, dest_dir_path);
    }

    // 复制数据库升级目录到输出目录
    copy_dir(project_root, "migrations", dest_dir_path);

    // 复制证书目录到输出目录
    copy_dir(project_root, "cert", dest_dir_path);
}

fn copy_dir(project_root: &str, src_dir: &str, dest_dir_path: &Path) {
    let src_dir_path = Path::new(project_root).join(src_dir);
    if !src_dir_path.exists() || !src_dir_path.is_dir() {
        return;
    }

    let dest_dir = dest_dir_path.join(src_dir);
    copy_dir_recursive(&src_dir_path, &dest_dir);
}

fn copy_dir_recursive(src_path: &Path, dest_path: &Path) {
    if src_path.is_dir() {
        fs::create_dir_all(dest_path).expect("Failed to create directory");

        for entry in fs::read_dir(src_path).expect("Failed to read directory") {
            let entry = entry.expect("Failed to read directory entry");
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let dest_entry_path = dest_path.join(entry_name);

            if entry_path.is_dir() {
                copy_dir_recursive(&entry_path, &dest_entry_path);
            } else if entry_path.is_file() {
                // 告诉 Cargo 当文件变化时重新运行 build.rs
                println!("cargo:rerun-if-changed={}", entry_path.display());
                // 复制文件到目的地
                fs::copy(&entry_path, &dest_entry_path).expect("Failed to copy file");
            }
        }
    }
}

/// 复制指定扩展名的配置文件到输出目录
///
/// 该函数会查找与指定文件名同名的配置文件（如 `jobx-svr.toml`），
/// 并将其从项目根目录复制到构建输出目录中。
///
/// # 参数
///
/// - `out_dir`: 构建输出目录路径
/// - `file_ext`: 配置文件的扩展名（如 "toml", "yml" 等）
///
/// # 行为
///
/// 1. 构造配置文件名：`{包名}.{扩展名}`
/// 2. 在项目根目录查找该配置文件
/// 3. 计算目标路径（输出目录向上三级目录）
/// 4. 如果源文件存在，则复制到目标路径
///
/// # Panics
///
/// - 当无法访问环境变量时
/// - 当路径操作失败时
/// - 当文件复制失败时
fn copy_file(project_root: &str, src_file_name: &str, src_file_ext: &str, dest_path: &Path) {
    // 获取源配置文件路径
    let src_file_name = format!("{src_file_name}.{src_file_ext}");

    let src_file_path = Path::new(project_root).join(&src_file_name);

    // 如果源文件存在，则执行复制操作
    if src_file_path.exists() {
        let dest_path = dest_path.join(&src_file_name);
        // println!("cargo:warning=copy {:?} to {:?}", src_file_path, dest_path);

        // 告诉 Cargo 当文件变化时重新运行 build.rs
        println!("cargo:rerun-if-changed={src_file_name}");
        // 复制文件到目的地
        fs::copy(src_file_path, dest_path).expect("Failed to copy app file");
    }
}

/// # 处理 jobx-web 前端产物的构建与变更监听
///
/// `rust-embed` 在**编译期**把 `../jobx-web/dist` 读进二进制，所以前端产物变化后必须
/// 重新编译才会生效。这里做三件事：
///
/// 1. 声明 `rerun-if-changed`：前端产物 / 构建配置变了就让 `cargo build` 重新编译，
///    避免改了前端但二进制里还是旧页面。
/// 2. 产物缺失且本地已装好依赖时，顺手跑一次 `vite build`（可用 `JOBX_WEB_SKIP_BUILD=1` 关掉）。
/// 3. 实在构建不了只发 `cargo:warning`，绝不 panic —— 没有 node 的环境照样能 `cargo build`，
///    只是页面会返回「未构建」提示（见 `src/web/assets.rs`）。
fn setup_web_frontend(project_root: &str) {
    let web_dir = Path::new(project_root).join("../jobx-web");
    let dist_dir = Path::new(project_root).join("../jobx-web/dist");

    // 前端产物目录整体监听（含新增/删除文件），保证重新 build 时带上最新页面
    println!("cargo:rerun-if-changed={}", dist_dir.display());
    // 构建配置也监听：换 base、换端口代理等都会改变产物
    for name in ["package.json", "vite.config.ts", "index.html", ".env.production"] {
        let path = web_dir.join(name);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    if std::env::var("JOBX_WEB_SKIP_BUILD").as_deref() == Ok("1") {
        return;
    }

    // 已经构建过就不再自动构建，避免每次 cargo build 都跑一遍 vite
    if dist_dir.join("index.html").exists() {
        return;
    }

    if !web_dir.join("package.json").exists() {
        println!(
            "cargo:warning=未找到 jobx-web 前端项目，页面将返回「未构建」提示；\
             如需页面请确认 jobx/jobx-web 目录存在"
        );
        return;
    }

    // 只在依赖已安装（node_modules/.bin/vite 存在）时才自动构建：
    // 不会悄悄联网装依赖，也不会在没网的环境里卡住。
    let vite_bin = web_dir.join("node_modules/.bin/vite");
    if !vite_bin.exists() {
        println!(
            "cargo:warning=jobx-web/dist 不存在。请先执行：cd jobx-web && pnpm install && pnpm build"
        );
        return;
    }

    println!("cargo:warning=jobx-web/dist 不存在，尝试自动构建前端（可用 JOBX_WEB_SKIP_BUILD=1 跳过）...");
    let status = Command::new(&vite_bin)
        .arg("build")
        .current_dir(&web_dir)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=jobx-web 前端构建完成 ✅");
        }
        _ => {
            println!(
                "cargo:warning=jobx-web 前端自动构建失败，请手动执行：cd jobx-web && pnpm run build"
            );
        }
    }
}