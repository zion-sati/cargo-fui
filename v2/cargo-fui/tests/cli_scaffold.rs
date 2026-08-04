use cargo_fui::{
    create_project, load_manifest, run_cli, ApplicationTarget, CliIo, Error, NewProjectOptions,
    ProjectTemplate,
};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fui-phase-3-{}-{}",
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
fn cargo_subcommand_parser_scaffolds_all_target_kinds() {
    let temp = TempDir::new();
    for target in ["native", "web", "universal"] {
        let output = RefCell::new(Vec::new());
        let io = CliIo::new(|message| output.borrow_mut().push(message.to_string()));
        run_cli(
            [
                "fui".to_string(),
                "new".to_string(),
                target.to_string(),
                "--target".to_string(),
                target.to_string(),
            ],
            Ok(temp.0.clone()),
            &io,
        )
        .unwrap();
        assert!(temp.0.join(target).join("Cargo.toml").is_file());
        assert!(temp.0.join(target).join("fui-config.json").is_file());
        assert!(output.borrow()[0].starts_with("Created "));
    }
}

#[test]
fn templates_keep_native_node_free_and_universal_boundaries_explicit() {
    let temp = TempDir::new();
    let native = temp.0.join("native-app");
    create(&native, ProjectTemplate::Native);
    assert!(!native.join("package.json").exists());
    assert!(!native.join("harness.ts").exists());
    assert!(native.join("src/services/native.rs").is_file());
    assert!(native.join("worker/src/lib.rs").is_file());
    assert!(!native.join("src/services/web.rs").exists());
    assert!(fs::read_to_string(native.join("Cargo.toml"))
        .unwrap()
        .contains("\n[workspace]\n"));
    assert_worker_is_native_only(&native.join("Cargo.toml"), "native_app_worker");
    let native_manifest = load_manifest(native.join("fui.toml")).unwrap();
    let native_config = cargo_fui::load_fui_config(native.join("fui-config.json")).unwrap();
    assert_eq!(native_config.version, 1);
    assert_eq!(native_manifest.workers[0].entries, ["sampleWorker"]);
    assert_eq!(
        native_manifest.application.targets,
        vec![ApplicationTarget::Native]
    );

    let web = temp.0.join("web-app");
    create(&web, ProjectTemplate::Web);
    assert!(web.join("package.json").is_file());
    assert!(web.join("harness.ts").is_file());
    assert!(!web.join("src/services/native.rs").exists());
    assert!(web.join("src/services/web.rs").is_file());
    assert!(web.join("worker/src/lib.rs").is_file());
    assert!(fs::read_to_string(web.join("Cargo.toml"))
        .unwrap()
        .contains("\n[workspace]\n"));
    assert_worker_is_native_only(&web.join("Cargo.toml"), "web_app_worker");
    assert!(web.join("loading-overlay-styles.html").is_file());
    assert!(web.join("loading-overlay-body.html").is_file());
    assert!(fs::read_to_string(web.join("index.html"))
        .unwrap()
        .contains("data-effindom-canvas-size-source"));

    let universal = temp.0.join("universal-app");
    create(&universal, ProjectTemplate::Universal);
    assert!(universal.join("crates/ui/src/services/native.rs").is_file());
    assert!(universal.join("crates/ui/src/services/web.rs").is_file());
    assert!(universal.join("crates/worker/src/lib.rs").is_file());
    assert_eq!(
        load_manifest(universal.join("fui.toml"))
            .unwrap()
            .application
            .targets,
        vec![ApplicationTarget::Native, ApplicationTarget::Web]
    );
    let cargo = fs::read_to_string(universal.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("crates/ui"));
    assert!(
        fs::read_to_string(universal.join("crates/native/Cargo.toml"))
            .unwrap()
            .contains("crate-type = [\"staticlib\"]")
    );
    assert!(fs::read_to_string(universal.join("crates/web/Cargo.toml"))
        .unwrap()
        .contains("crate-type = [\"cdylib\"]"));
    let source = fs::read_to_string(universal.join("crates/ui/src/lib.rs")).unwrap();
    assert!(source.contains("ui!"));
    assert!(source.contains("Application::caption"));
    assert!(source.contains("Worker::new(\"./workers.wasm\", \"sampleWorker\")"));
    assert!(!source.contains("extern \"C\" fn __runApp"));
    assert!(universal.join("loading-overlay-styles.html").is_file());
    assert_png(&universal.join("assets/application-icon.png"));
}

fn assert_worker_is_native_only(manifest: &Path, dependency: &str) {
    const NATIVE_DEPENDENCIES: &str =
        "\n[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\n";
    let cargo = fs::read_to_string(manifest).unwrap();
    let (wasm_visible, native_only) = cargo.split_once(NATIVE_DEPENDENCIES).unwrap();
    assert!(!wasm_visible.contains(dependency));
    assert!(native_only.contains(dependency));
}

#[test]
fn templates_generate_target_specific_first_run_guidance() {
    let temp = TempDir::new();

    let native = temp.0.join("native-readme");
    create(&native, ProjectTemplate::Native);
    let native_readme = fs::read_to_string(native.join("README.md")).unwrap();
    assert!(native_readme.contains("without Electron or a WebView"));
    assert!(native_readme.contains("cargo fui package"));
    assert!(native_readme.contains("src/services/native.rs"));
    assert!(native_readme.contains("shared Worker implementation used by native and web builds"));
    assert!(!native_readme.contains("Node.js LTS"));

    let web = temp.0.join("web-readme");
    create(&web, ProjectTemplate::Web);
    let web_readme = fs::read_to_string(web.join("README.md")).unwrap();
    assert!(web_readme.contains("Node.js LTS"));
    assert!(web_readme.contains("intentionally unavailable for web-only projects"));
    assert!(web_readme.contains("src/services/web.rs"));
    assert!(web_readme.contains("Worker implementation compiled to WebAssembly"));

    let universal = temp.0.join("universal-readme");
    create(&universal, ProjectTemplate::Universal);
    let universal_readme = fs::read_to_string(universal.join("README.md")).unwrap();
    assert!(universal_readme.contains("shared retained UI"));
    assert!(universal_readme.contains("crates/ui"));
    assert!(universal_readme.contains("Produce optimized native and browser builds"));
    assert!(universal_readme
        .contains("Worker implementation linked natively or compiled to WebAssembly"));

    for readme in [native_readme, web_readme, universal_readme] {
        assert!(readme.contains("https://rustup.rs/"));
        assert!(readme.contains("cargo install --locked cargo-fui"));
        assert!(readme.contains("https://fui-rs-demo.effindom.dev/"));
    }
}

#[test]
fn scaffolding_rejects_nonempty_destinations_without_overwriting() {
    let temp = TempDir::new();
    let destination = temp.0.join("occupied");
    fs::create_dir_all(&destination).unwrap();
    fs::write(destination.join("keep.txt"), "owned by user").unwrap();
    let error = create_project(&NewProjectOptions {
        destination: destination.clone(),
        project_name: "occupied".into(),
        template: ProjectTemplate::Native,
    })
    .unwrap_err();
    assert!(matches!(error, Error::ProjectExists(path) if path == destination));
    assert_eq!(
        fs::read_to_string(destination.join("keep.txt")).unwrap(),
        "owned by user"
    );
}

#[test]
fn cli_reports_unknown_commands_and_missing_projects() {
    let temp = TempDir::new();
    let io = CliIo::new(|_| {});
    let unknown = run_cli(["fui".into(), "unknown".into()], Ok(temp.0.clone()), &io).unwrap_err();
    assert!(unknown.to_string().contains("unknown command"));
    let missing = run_cli(["fui".into(), "build".into()], Ok(temp.0.clone()), &io).unwrap_err();
    assert!(matches!(missing, Error::ProjectNotFound(_)));
}

fn create(destination: &Path, template: ProjectTemplate) {
    create_project(&NewProjectOptions {
        destination: destination.to_path_buf(),
        project_name: destination
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        template,
    })
    .unwrap();
}

fn assert_png(path: &Path) {
    let image = image::open(path).unwrap();
    assert_eq!((image.width(), image.height()), (256, 256));
}
