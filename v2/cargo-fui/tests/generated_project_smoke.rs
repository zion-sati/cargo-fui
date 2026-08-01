use cargo_fui::{create_project, NewProjectOptions, ProjectTemplate};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fui-generated-smoke-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generated_rust_targets_are_format_clean_and_compile() {
    let temp = TempDir::new();
    let fui_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fui-rs");
    for template in [
        ProjectTemplate::Native,
        ProjectTemplate::Web,
        ProjectTemplate::Universal,
    ] {
        let name = match template {
            ProjectTemplate::Native => "native",
            ProjectTemplate::Web => "web",
            ProjectTemplate::Universal => "universal",
        };
        let root = temp.0.join(name);
        create_project(&NewProjectOptions {
            destination: root.clone(),
            project_name: format!("{name}-smoke"),
            template,
        })
        .unwrap();
        if fui_rs.is_dir() {
            replace_fui_dependencies(&root, &fui_rs);
        }
        assert_success(
            Command::new("cargo")
                .current_dir(&root)
                .args(["fmt", "--check"]),
            "cargo fmt",
        );
        if template == ProjectTemplate::Universal {
            assert_success(
                Command::new("cargo").current_dir(&root).args([
                    "check",
                    "--manifest-path",
                    "crates/native/Cargo.toml",
                ]),
                "cargo check native adapter",
            );
            assert_success(
                Command::new("cargo").current_dir(&root).args([
                    "check",
                    "--manifest-path",
                    "crates/web/Cargo.toml",
                ]),
                "cargo check web adapter",
            );
        } else {
            let mut check = Command::new("cargo");
            check.current_dir(&root).arg("check");
            if template == ProjectTemplate::Native {
                check.args(["--features", "native"]);
            }
            assert_success(&mut check, "cargo check");
        }
    }
}

fn replace_fui_dependencies(root: &Path, fui_rs: &Path) {
    let replacement = format!("fui = {{ package = \"fui-rs\", path = {:?} }}", fui_rs);
    let worker_replacement = format!(
        "fui = {{ package = \"fui-rs\", path = {:?}, default-features = false, features = [\"worker-runtime\"] }}",
        fui_rs
    );
    for path in cargo_manifests(root) {
        let cargo = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| {
                if line
                    .trim_start()
                    .starts_with("fui = { package = \"fui-rs\", version = ")
                {
                    if line.contains("worker-runtime") {
                        worker_replacement.as_str()
                    } else {
                        replacement.as_str()
                    }
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        fs::write(path, cargo).unwrap();
    }
}

fn cargo_manifests(root: &Path) -> Vec<PathBuf> {
    let mut output = Vec::new();
    collect_manifests(root, &mut output);
    output
}

fn collect_manifests(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_manifests(&path, output);
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            output.push(path);
        }
    }
}

fn assert_success(command: &mut Command, label: &str) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{label} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
