use std::fs;
use std::path::Path;

fn sync_dir(src: &Path, dst: &Path) {
    let Ok(entries) = fs::read_dir(src) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            let _ = fs::create_dir_all(&dst_path);
            sync_dir(&src_path, &dst_path);
        } else if file_type.is_file() {
            let src_bytes = fs::read(&src_path).unwrap_or_default();
            let needs_copy = fs::read(&dst_path).map_or(true, |dst_bytes| dst_bytes != src_bytes);
            if needs_copy {
                if let Some(parent) = dst_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(&dst_path, &src_bytes);
            }
        }
    }
}

fn rerun_if_changed_recursive(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            rerun_if_changed_recursive(&path);
        } else if file_type.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn main() {
    let scripts_src = Path::new("../src/vs/workbench/contrib/terminal/common/scripts");
    let scripts_dst = Path::new("shell-integration");

    if scripts_src.exists() {
        fs::create_dir_all(scripts_dst).expect("Failed to create shell-integration dir");

        let shell_files = [
            "shellIntegration-rc.zsh",
            "shellIntegration-env.zsh",
            "shellIntegration-profile.zsh",
            "shellIntegration-login.zsh",
            "shellIntegration-bash.sh",
            "shellIntegration.fish",
            "shellIntegration.ps1",
        ];

        for file in &shell_files {
            let src = scripts_src.join(file);
            let dst = scripts_dst.join(file);
            if src.exists() {
                let src_bytes =
                    fs::read(&src).unwrap_or_else(|e| panic!("Failed to read {file}: {e}"));
                let needs_copy = fs::read(&dst).map_or(true, |dst_bytes| dst_bytes != src_bytes);
                if needs_copy {
                    fs::write(&dst, &src_bytes)
                        .unwrap_or_else(|e| panic!("Failed to copy {file}: {e}"));
                }
                println!("cargo:rerun-if-changed={}", src.display());
            }
        }
    }

    let ext_host_src = Path::new("extension-host");
    let ext_host_targets = [
        Path::new("target/debug/extension-host"),
        Path::new("target/release/extension-host"),
    ];
    if ext_host_src.exists() {
        for ext_host_dst in &ext_host_targets {
            let _ = fs::create_dir_all(ext_host_dst);
            sync_dir(ext_host_src, ext_host_dst);
        }
        rerun_if_changed_recursive(ext_host_src);
    }

    tauri_build::build();
}
