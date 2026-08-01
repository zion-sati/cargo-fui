use cargo_fui::{
    load_manifest, resolve_package_contract, Architecture, BuildProfile, Error, OperatingSystem,
    PackageRequest, SigningMode, CORE_ABI_VERSION, UI_ABI_VERSION,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(cargo: &str, fui: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("cargo-fui-phase-1-1-{}-{id}", std::process::id()));
        fs::create_dir_all(&root).expect("create test project");
        fs::write(root.join("Cargo.toml"), cargo).expect("write Cargo.toml");
        fs::write(root.join("fui.toml"), fui).expect("write fui.toml");
        Self { root }
    }

    fn manifest(&self) -> PathBuf {
        self.root.join("fui.toml")
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove test project");
    }
}

const CARGO: &str = r#"
[package]
name = "sample-app"
version = "1.2.3-alpha.1"
edition = "2021"
"#;

fn request(target: &str) -> PackageRequest {
    PackageRequest::new(target, BuildProfile::Release, SigningMode::Unsigned)
}

#[test]
fn resolves_cargo_owned_identity_and_macos_layout() {
    let project = TestProject::new(
        CARGO,
        r#"
schema-version = 1

[application]
identifier = "dev.effindom.sample"
icon = "assets/app-icon.svg"

[assets]
sources = ["assets", "shared"]

[package.macos]
minimum-version = "13.0"
"#,
    );
    let contract = resolve_package_contract(project.manifest(), request("aarch64-apple-darwin"))
        .expect("resolve package contract");

    assert_eq!(contract.application.name, "sample-app");
    assert_eq!(contract.application.version, "1.2.3-alpha.1");
    assert_eq!(contract.application.caption, "sample-app");
    assert_eq!(contract.target.operating_system, OperatingSystem::MacOs);
    assert_eq!(contract.target.architecture, Architecture::Arm64);
    assert_eq!(contract.layout.root, Path::new("sample-app.app"));
    assert_eq!(
        contract.layout.executable,
        Path::new("sample-app.app/Contents/MacOS/sample-app")
    );
    assert_eq!(
        contract.layout.runtime_libraries,
        Path::new("sample-app.app/Contents/Frameworks")
    );
    assert_eq!(
        contract.layout.library_search,
        cargo_fui::NativeLibrarySearch::LoaderRelative("@loader_path/../Frameworks".to_owned())
    );
    assert_eq!(contract.runtime_abi.core, CORE_ABI_VERSION);
    assert_eq!(contract.runtime_abi.ui, UI_ABI_VERSION);
    assert_eq!(contract.application.asset_sources.len(), 2);
    let metadata = contract.native_metadata().expect("resolve native metadata");
    assert_eq!(metadata.executable_name, "sample-app");
    assert_eq!(
        metadata.native_version,
        cargo_fui::NativeVersion::new(1, 2, 3, 0)
    );
}

#[test]
fn resolves_windows_and_linux_stable_layouts() {
    let project = TestProject::new(
        CARGO,
        r#"
[application]
identifier = "dev.effindom.sample"
caption = "Sample Application"

[package.windows]
publisher = "CN=EffinDOM"

[package.linux]
categories = ["Utility", "Development"]
"#,
    );
    let windows = resolve_package_contract(
        project.manifest(),
        PackageRequest::new(
            "x86_64-pc-windows-msvc",
            BuildProfile::Release,
            SigningMode::Signed,
        ),
    )
    .expect("resolve Windows package");
    assert_eq!(windows.application.caption, "Sample Application");
    assert_eq!(
        windows.layout.executable,
        Path::new("sample-app/sample-app.exe")
    );
    assert_eq!(
        windows.layout.runtime_resources,
        Path::new("sample-app/assets/effindom")
    );
    assert_eq!(
        windows.layout.library_search,
        cargo_fui::NativeLibrarySearch::ExecutableDirectory
    );

    let linux = resolve_package_contract(project.manifest(), request("aarch64-unknown-linux-gnu"))
        .expect("resolve Linux package");
    assert_eq!(
        linux.layout.executable,
        Path::new("sample-app/bin/sample-app")
    );
    assert_eq!(linux.layout.runtime_libraries, Path::new("sample-app/lib"));
    assert_eq!(
        linux.layout.library_search,
        cargo_fui::NativeLibrarySearch::LoaderRelative("$ORIGIN/../lib".to_owned())
    );
    assert_eq!(
        linux.layout.runtime_resources,
        Path::new("sample-app/share/effindom")
    );
}

#[test]
fn package_record_contains_target_modes_and_abi() {
    let project = TestProject::new(
        CARGO,
        r#"
[application]
identifier = "dev.effindom.sample"
"#,
    );
    let contract = resolve_package_contract(
        project.manifest(),
        PackageRequest::new(
            "x86_64-apple-darwin",
            BuildProfile::Debug,
            SigningMode::Unsigned,
        ),
    )
    .expect("resolve package contract");
    let record = contract.to_pretty_json().expect("serialize package record");
    assert!(record.contains("\"triple\": \"x86_64-apple-darwin\""));
    assert!(record.contains("\"profile\": \"debug\""));
    assert!(record.contains("\"signing\": \"unsigned\""));
    assert!(record.contains("\"core\": 2"));
    assert!(record.contains("\"ui\": 1"));
    let metadata = contract.package_metadata();
    assert_eq!(metadata.application_identifier, "dev.effindom.sample");
    assert_eq!(metadata.target_triple, "x86_64-apple-darwin");
    assert_eq!(metadata.core_abi, 2);
    assert_eq!(metadata.ui_abi, 1);
}

#[test]
fn rejects_virtual_workspace_manifest_with_actionable_error() {
    let project = TestProject::new(
        "[workspace]\nmembers = []\n",
        r#"
[application]
identifier = "dev.effindom.sample"
"#,
    );
    let error = resolve_package_contract(project.manifest(), request("aarch64-apple-darwin"))
        .expect_err("virtual workspace must fail");
    assert!(matches!(error, Error::MissingCargoPackage { .. }));
    assert!(error.to_string().contains("application.cargo-manifest"));
}

#[test]
fn rejects_unknown_schema_fields_and_unsupported_versions() {
    let unknown = TestProject::new(
        CARGO,
        r#"
[application]
identifier = "dev.effindom.sample"
surprise = true
"#,
    );
    assert!(matches!(
        resolve_package_contract(unknown.manifest(), request("aarch64-apple-darwin")),
        Err(Error::ParseFuiManifest { .. })
    ));

    let unsupported = TestProject::new(
        CARGO,
        r#"
schema-version = 2
[application]
identifier = "dev.effindom.sample"
"#,
    );
    assert!(matches!(
        resolve_package_contract(unsupported.manifest(), request("aarch64-apple-darwin")),
        Err(Error::UnsupportedSchemaVersion {
            found: 2,
            supported: 1
        })
    ));
}

#[test]
fn rejects_invalid_identifiers_icons_targets_and_signing_metadata() {
    let invalid_identifier = TestProject::new(CARGO, "[application]\nidentifier = \"sample\"\n");
    assert!(matches!(
        resolve_package_contract(
            invalid_identifier.manifest(),
            request("aarch64-apple-darwin")
        ),
        Err(Error::InvalidApplicationIdentifier(_))
    ));

    let invalid_icon = TestProject::new(
        CARGO,
        "[application]\nidentifier = \"dev.effindom.sample\"\nicon = \"icon.jpg\"\n",
    );
    assert!(matches!(
        resolve_package_contract(invalid_icon.manifest(), request("aarch64-apple-darwin")),
        Err(Error::InvalidIconPath(_))
    ));

    let unsupported_target = TestProject::new(
        CARGO,
        "[application]\nidentifier = \"dev.effindom.sample\"\n",
    );
    assert!(matches!(
        resolve_package_contract(
            unsupported_target.manifest(),
            request("wasm32-unknown-unknown")
        ),
        Err(Error::UnsupportedTarget(_))
    ));

    let unsigned_windows_only = TestProject::new(
        CARGO,
        "[application]\nidentifier = \"dev.effindom.sample\"\n",
    );
    let error = resolve_package_contract(
        unsigned_windows_only.manifest(),
        PackageRequest::new(
            "x86_64-pc-windows-msvc",
            BuildProfile::Release,
            SigningMode::Signed,
        ),
    )
    .expect_err("signed Windows package must require publisher");
    assert!(matches!(error, Error::MissingSigningMetadata { .. }));
}

#[test]
fn parses_explicit_worker_bundle_contract() {
    let project = TestProject::new(
        CARGO,
        r#"
[application]
identifier = "dev.effindom.sample"

[[workers]]
id = "compute"
web-artifact = "dist/workers.wasm"
native-cargo-manifest = "workers/Cargo.toml"
entries = ["findPrimes", "hashFile"]
host-services = ["appWorkerClockWallClockSinceEpochMs"]
"#,
    );
    let manifest = load_manifest(project.manifest()).expect("load worker manifest");
    assert_eq!(manifest.workers.len(), 1);
    assert_eq!(manifest.workers[0].id, "compute");
    assert_eq!(manifest.workers[0].entries, ["findPrimes", "hashFile"]);
}

#[test]
fn rejects_duplicate_worker_bundles_entries_and_services() {
    for workers in [
        r#"
[[workers]]
id = "compute"
web-artifact = "a.wasm"
native-cargo-manifest = "a/Cargo.toml"
entries = ["first"]
[[workers]]
id = "compute"
web-artifact = "b.wasm"
native-cargo-manifest = "b/Cargo.toml"
entries = ["second"]
"#,
        r#"
[[workers]]
id = "first"
web-artifact = "a.wasm"
native-cargo-manifest = "a/Cargo.toml"
entries = ["shared"]
[[workers]]
id = "second"
web-artifact = "b.wasm"
native-cargo-manifest = "b/Cargo.toml"
entries = ["shared"]
"#,
        r#"
[[workers]]
id = "compute"
web-artifact = "a.wasm"
native-cargo-manifest = "a/Cargo.toml"
entries = ["first"]
host-services = ["clock", "clock"]
"#,
    ] {
        let project = TestProject::new(
            CARGO,
            &format!("[application]\nidentifier = \"dev.effindom.sample\"\n{workers}"),
        );
        assert!(matches!(
            load_manifest(project.manifest()),
            Err(Error::InvalidWorkerManifest(_))
        ));
    }
}

#[test]
fn rejects_missing_or_malformed_worker_fields() {
    for workers in [
        r#"
[[workers]]
id = "compute"
native-cargo-manifest = "workers/Cargo.toml"
entries = ["findPrimes"]
"#,
        r#"
[[workers]]
id = "compute"
web-artifact = "workers.wasm"
native-cargo-manifest = "workers/Cargo.toml"
entries = "findPrimes"
"#,
    ] {
        let project = TestProject::new(
            CARGO,
            &format!("[application]\nidentifier = \"dev.effindom.sample\"\n{workers}"),
        );
        assert!(matches!(
            load_manifest(project.manifest()),
            Err(Error::ParseFuiManifest { .. })
        ));
    }
}

#[test]
fn rejects_worker_bundle_without_entries() {
    let project = TestProject::new(
        CARGO,
        r#"
[application]
identifier = "dev.effindom.sample"
[[workers]]
id = "compute"
web-artifact = "workers.wasm"
native-cargo-manifest = "workers/Cargo.toml"
entries = []
"#,
    );
    assert!(matches!(
        load_manifest(project.manifest()),
        Err(Error::InvalidWorkerManifest(_))
    ));
}

#[test]
fn rejects_unsafe_or_non_wasm_worker_artifact_paths() {
    for artifact in ["../workers.wasm", "/tmp/workers.wasm", "workers.bin"] {
        let project = TestProject::new(
            CARGO,
            &format!(
                "[application]\nidentifier = \"dev.effindom.sample\"\n[[workers]]\nid = \"compute\"\nweb-artifact = {artifact:?}\nnative-cargo-manifest = \"workers/Cargo.toml\"\nentries = [\"findPrimes\"]\n"
            ),
        );
        assert!(matches!(
            load_manifest(project.manifest()),
            Err(Error::InvalidWorkerManifest(_))
        ));
    }
}
