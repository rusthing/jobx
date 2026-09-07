use std::fs;
use std::path::Path;

fn main() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_dir_path = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("无法找到目标目录");

    let config_files = vec!["jobx-svr.toml", "log.toml", ".env"];
    for file in &config_files {
        let src = Path::new(project_root).join(file);
        if src.exists() {
            let dest = dest_dir_path.join(file);
            let _ = fs::copy(&src, &dest);
        }
    }
}
