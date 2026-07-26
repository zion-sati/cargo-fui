use cargo_fui::{
    resolve_package_contract, stage_native_bundle, Architecture, BuildProfile, BundleFileRole,
    NativeBuildOutput, NativeLibraryOutput, OverwritePolicy, PackageRequest, SigningMode,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fui-native-bundle-{}-{}",
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
fn stages_explicit_build_outputs_through_the_resolved_platform_layout() {
    let temp = TempDir::new();
    let project = temp.0.join("project");
    fs::create_dir_all(project.join("resources")).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='sample-app'\nversion='1.2.3'\n",
    )
    .unwrap();
    fs::write(
        project.join("fui.toml"),
        "schema-version=1\n[application]\nidentifier='dev.effindom.sample'\ncaption='Sample'\n",
    )
    .unwrap();
    let build = temp.0.join("build");
    let runtime_resources = build.join("runtime");
    let app_resources = build.join("app");
    fs::create_dir_all(runtime_resources.join("fonts")).unwrap();
    fs::create_dir_all(&app_resources).unwrap();
    write(&build.join("sample"), "executable");
    write(&build.join("effindom.so"), "runtime");
    write(&build.join("libSDL3.so"), "third-party");
    write(&runtime_resources.join("fonts/body.ttf"), "font");
    write(&app_resources.join("texture.png"), "texture");
    let contract = resolve_package_contract(
        project.join("fui.toml"),
        PackageRequest::new(
            "x86_64-unknown-linux-gnu",
            BuildProfile::Release,
            SigningMode::Unsigned,
        ),
    )
    .unwrap();
    assert_eq!(contract.target.architecture, Architecture::X64);
    let staged = stage_native_bundle(
        &contract,
        &NativeBuildOutput {
            application_executable: build.join("sample"),
            effindom_runtime_libraries: vec![
                NativeLibraryOutput::from_file(build.join("effindom.so")).unwrap(),
                NativeLibraryOutput::new(build.join("libSDL3.so"), "plugins/libSDL3.so"),
            ],
            third_party_libraries: Vec::new(),
            runtime_resources,
            application_resources: app_resources,
        },
        temp.0.join("packages"),
        OverwritePolicy::Reject,
    )
    .unwrap();
    assert!(staged.root.join("bin/sample-app").is_file());
    assert!(staged.root.join("lib/effindom.so").is_file());
    assert!(staged.root.join("lib/plugins/libSDL3.so").is_file());
    assert!(staged.root.join("lib/plugins/libSDL3.so.0").is_file());
    assert!(staged.root.join("share/effindom/fonts/body.ttf").is_file());
    assert!(staged.root.join("share/app/texture.png").is_file());
    assert!(staged.root.join("share/effindom-package.json").is_file());
    assert!(staged
        .record
        .files
        .iter()
        .any(|file| file.role == BundleFileRole::ApplicationExecutable));
}

#[test]
fn rejects_missing_resource_roots_without_publishing_a_partial_bundle() {
    let temp = TempDir::new();
    let project = temp.0.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='sample-app'\nversion='1.0.0'\n",
    )
    .unwrap();
    fs::write(
        project.join("fui.toml"),
        "schema-version=1\n[application]\nidentifier='dev.effindom.sample'\n",
    )
    .unwrap();
    let executable = temp.0.join("sample");
    write(&executable, "executable");
    let contract = resolve_package_contract(
        project.join("fui.toml"),
        PackageRequest::new(
            "x86_64-unknown-linux-gnu",
            BuildProfile::Release,
            SigningMode::Unsigned,
        ),
    )
    .unwrap();
    let destination = temp.0.join("packages");
    let error = stage_native_bundle(
        &contract,
        &NativeBuildOutput {
            application_executable: executable,
            effindom_runtime_libraries: Vec::new(),
            third_party_libraries: Vec::new(),
            runtime_resources: temp.0.join("missing-runtime"),
            application_resources: temp.0.join("missing-app"),
        },
        &destination,
        OverwritePolicy::Reject,
    )
    .unwrap_err();
    assert!(error.to_string().contains("read native build resources"));
    assert!(!destination.join("sample-app").exists());
}

#[test]
fn rejects_library_destinations_that_escape_the_runtime_directory() {
    let temp = TempDir::new();
    let project = temp.0.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='sample-app'\nversion='1.0.0'\n",
    )
    .unwrap();
    fs::write(
        project.join("fui.toml"),
        "schema-version=1\n[application]\nidentifier='dev.effindom.sample'\n",
    )
    .unwrap();
    let build = temp.0.join("build");
    let runtime_resources = build.join("runtime");
    let application_resources = build.join("app");
    fs::create_dir_all(&runtime_resources).unwrap();
    fs::create_dir_all(&application_resources).unwrap();
    write(&build.join("sample"), "executable");
    write(&build.join("plugin.so"), "plugin");
    let contract = resolve_package_contract(
        project.join("fui.toml"),
        PackageRequest::new(
            "x86_64-unknown-linux-gnu",
            BuildProfile::Release,
            SigningMode::Unsigned,
        ),
    )
    .unwrap();
    let destination = temp.0.join("packages");
    let error = stage_native_bundle(
        &contract,
        &NativeBuildOutput {
            application_executable: build.join("sample"),
            effindom_runtime_libraries: Vec::new(),
            third_party_libraries: vec![NativeLibraryOutput::new(
                build.join("plugin.so"),
                "../plugin.so",
            )],
            runtime_resources,
            application_resources,
        },
        &destination,
        OverwritePolicy::Reject,
    )
    .unwrap_err();
    assert!(error.to_string().contains("must be package-relative"));
    assert!(!destination.join("sample-app").exists());
}

fn write(path: &Path, value: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, value).unwrap();
}
