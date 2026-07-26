use cargo_fui::{
    acquire_native_runtime, clean_native_runtime_cache, list_native_runtime_cache,
    runtime_requirement_from_cargo_metadata, Error, NativeRuntimeAcquisition,
    NativeRuntimeReleaseManifest, NativeRuntimeSource, NativeRuntimeTarget, RuntimeDownloader,
    RuntimeRequirement, NATIVE_RUNTIME_MANIFEST_SCHEMA_VERSION, REQUIRED_NATIVE_RUNTIME_TARGETS,
};
use effindom_native_packaging::{
    create_native_runtime_artifact, encode_native_runtime_release_manifest,
    NativeRuntimeArtifactInput, NativeRuntimeArtifactRequest, NativeRuntimeFileRole,
    NativeRuntimeMinimumOs, OverwritePolicy,
};
use semver::Version;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
    downloads: BTreeMap<String, Vec<u8>>,
    override_root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cargo-fui-runtime-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("input")).unwrap();
        fs::create_dir_all(root.join("artifacts")).unwrap();
        fs::write(root.join("input/runtime.bin"), b"runtime payload").unwrap();
        let mut artifacts = Vec::new();
        let mut roots = BTreeMap::new();
        for target in REQUIRED_NATIVE_RUNTIME_TARGETS {
            let output = create_native_runtime_artifact(
                &NativeRuntimeArtifactRequest {
                    schema_version: NATIVE_RUNTIME_MANIFEST_SCHEMA_VERSION,
                    source_commit: COMMIT.to_string(),
                    target,
                    core_abi: 2,
                    ui_abi: 1,
                    minimum_os: NativeRuntimeMinimumOs {
                        family: family(target).to_string(),
                        version: "1.0".to_string(),
                    },
                    destination: root.join(format!("artifacts/{}", target.as_str())),
                    files: vec![NativeRuntimeArtifactInput {
                        source: root.join("input/runtime.bin"),
                        path: "runtime/lib/runtime.bin".to_string(),
                        role: NativeRuntimeFileRole::RuntimeLibrary,
                        executable: false,
                    }],
                },
                OverwritePolicy::Reject,
            )
            .unwrap();
            roots.insert(target, output.root.clone());
            artifacts.push(output.artifact);
        }
        let manifest = NativeRuntimeReleaseManifest {
            schema_version: NATIVE_RUNTIME_MANIFEST_SCHEMA_VERSION,
            release: "1.2.3".to_string(),
            source_commit: COMMIT.to_string(),
            artifacts,
        };
        let base = "https://runtime.invalid";
        let mut downloads = BTreeMap::new();
        downloads.insert(
            format!("{base}/v1.2.3/native-runtime-manifest.json"),
            encode_native_runtime_release_manifest(&manifest).unwrap(),
        );
        for artifact in &manifest.artifacts {
            downloads.insert(
                format!("{base}/v1.2.3/{}", artifact.archive),
                fs::read(roots[&artifact.target].join(&artifact.archive)).unwrap(),
            );
        }
        let override_root = root.join("override");
        effindom_native_packaging::extract_native_runtime_artifact(
            &roots[&NativeRuntimeTarget::MacosArm64],
            &override_root,
            COMMIT,
            manifest.artifact(NativeRuntimeTarget::MacosArm64).unwrap(),
            OverwritePolicy::Reject,
        )
        .unwrap();
        Self {
            root,
            downloads,
            override_root,
        }
    }

    fn request(&self, offline: bool) -> NativeRuntimeAcquisition {
        NativeRuntimeAcquisition {
            requirement: RuntimeRequirement {
                release: Version::parse("1.2.3").unwrap(),
                core_abi: 2,
                ui_abi: 1,
            },
            target: NativeRuntimeTarget::MacosArm64,
            cache_root: self.root.join("cache"),
            override_root: None,
            offline,
            release_base_url: "https://runtime.invalid".to_string(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

impl RuntimeDownloader for Fixture {
    fn download(&self, url: &str) -> cargo_fui::Result<Vec<u8>> {
        self.downloads
            .get(url)
            .cloned()
            .ok_or_else(|| Error::RuntimeDownload {
                url: url.to_string(),
                message: "fixture URL not found".to_string(),
            })
    }
}

#[test]
fn downloads_reuses_lists_and_cleans_a_verified_cache() {
    let fixture = Fixture::new();
    let downloaded = acquire_native_runtime(&fixture.request(false), &fixture).unwrap();
    assert_eq!(downloaded.source, NativeRuntimeSource::Download);
    assert_eq!(
        fs::read(downloaded.root.join("runtime/lib/runtime.bin")).unwrap(),
        b"runtime payload"
    );
    let cached = acquire_native_runtime(&fixture.request(true), &fixture).unwrap();
    assert_eq!(cached.source, NativeRuntimeSource::Cache);
    assert_eq!(
        list_native_runtime_cache(fixture.root.join("cache"))
            .unwrap()
            .len(),
        1
    );
    clean_native_runtime_cache(
        fixture.root.join("cache"),
        Some(&Version::parse("1.2.3").unwrap()),
    )
    .unwrap();
    assert!(list_native_runtime_cache(fixture.root.join("cache"))
        .unwrap()
        .is_empty());
}

#[test]
fn supports_override_and_rejects_offline_missing_corrupt_and_abi_mismatch() {
    let fixture = Fixture::new();
    let mut request = fixture.request(false);
    request.override_root = Some(fixture.override_root.clone());
    assert_eq!(
        acquire_native_runtime(&request, &fixture).unwrap().source,
        NativeRuntimeSource::Override
    );
    request.requirement.core_abi = 99;
    assert!(matches!(
        acquire_native_runtime(&request, &fixture),
        Err(Error::RuntimeRequirement(_))
    ));
    assert!(matches!(
        acquire_native_runtime(&fixture.request(true), &fixture),
        Err(Error::RuntimeUnavailable(_))
    ));
    let acquired = acquire_native_runtime(&fixture.request(false), &fixture).unwrap();
    fs::write(acquired.root.join("runtime/lib/runtime.bin"), b"corrupt").unwrap();
    assert!(acquire_native_runtime(&fixture.request(true), &fixture).is_err());
    let recovered = acquire_native_runtime(&fixture.request(false), &fixture).unwrap();
    assert_eq!(recovered.source, NativeRuntimeSource::Download);
    assert_eq!(
        fs::read(recovered.root.join("runtime/lib/runtime.bin")).unwrap(),
        b"runtime payload"
    );
}

#[test]
fn resolves_exact_runtime_requirement_from_fui_rs_cargo_metadata() {
    let requirement = runtime_requirement_from_cargo_metadata(
        br#"{"packages":[{"name":"app","metadata":{}},{"name":"fui-rs","metadata":{"effindom":{"runtime-version":"2.3.4","core-abi":2,"ui-abi":1}}}]}"#,
    )
    .unwrap();
    assert_eq!(requirement.release, Version::parse("2.3.4").unwrap());
    assert_eq!((requirement.core_abi, requirement.ui_abi), (2, 1));
    assert!(runtime_requirement_from_cargo_metadata(br#"{"packages":[]}"#).is_err());
}

fn family(target: NativeRuntimeTarget) -> &'static str {
    match target {
        NativeRuntimeTarget::MacosArm64 | NativeRuntimeTarget::MacosX64 => "macos",
        NativeRuntimeTarget::WindowsArm64 | NativeRuntimeTarget::WindowsX64 => "windows",
        NativeRuntimeTarget::LinuxArm64 | NativeRuntimeTarget::LinuxX64 => "glibc",
    }
}
