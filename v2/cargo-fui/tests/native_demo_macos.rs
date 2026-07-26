#![cfg(target_os = "macos")]

use cargo_fui::{
    create_dmg, create_release_archive, extract_release_archive, resolve_package_contract,
    sign_macos_application, stage_native_bundle, ArtifactSigningPurpose, BuildProfile, DmgInputs,
    MacOsSigningInputs, NativeBuildOutput, NativeLibraryOutput, OverwritePolicy, PackageRequest,
    ReleaseArchiveFormat, ReleaseArchiveSpec, SigningMode,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fui-native-demo-macos-{}-{}",
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
fn packages_relocates_signs_and_launches_the_real_native_demo() {
    let Some(output_root) = std::env::var_os("EFFINDOM_NATIVE_DEMO_OUTPUT") else {
        eprintln!("skipped: set EFFINDOM_NATIVE_DEMO_OUTPUT for real macOS package acceptance");
        return;
    };
    let Some(manifest) = std::env::var_os("EFFINDOM_NATIVE_DEMO_MANIFEST") else {
        eprintln!("skipped: set EFFINDOM_NATIVE_DEMO_MANIFEST for real macOS package acceptance");
        return;
    };
    let output_root = PathBuf::from(output_root);
    let source_app = output_root.join("effindom_v2_macos_native.app");
    let source_frameworks = source_app.join("Contents/Frameworks");
    let source_resources = source_app.join("Contents/Resources");
    let (effindom_libraries, third_party_libraries) = libraries(&source_frameworks);
    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let contract = resolve_package_contract(
        PathBuf::from(manifest),
        PackageRequest::new(target, BuildProfile::Release, SigningMode::Unsigned),
    )
    .expect("resolve real native demo package contract");
    let temp = TempDir::new();
    let staged = stage_native_bundle(
        &contract,
        &NativeBuildOutput {
            application_executable: source_app.join("Contents/MacOS/effindom_v2_macos_native"),
            effindom_runtime_libraries: library_outputs(effindom_libraries),
            third_party_libraries: library_outputs(third_party_libraries),
            runtime_resources: source_resources.join("effindom"),
            application_resources: source_resources.join("app"),
        },
        temp.0.join("staged"),
        OverwritePolicy::Reject,
    )
    .expect("stage real native demo");

    assert_macos_metadata(&staged.root);
    assert_loader_paths_are_relocatable(&staged.root);
    launch_to_screenshot(&staged.root, &temp.0.join("staged.png"));

    let archive_spec = ReleaseArchiveSpec {
        archive_name: "effindom-native-demo.app.tar.zst".to_string(),
        format: ReleaseArchiveFormat::TarZstd,
        package_record: staged.package_record.clone(),
    };
    let archive = create_release_archive(
        &staged.root,
        temp.0.join("archive"),
        &archive_spec,
        OverwritePolicy::Reject,
    )
    .expect("archive real native demo");
    let relocated = temp.0.join("unrelated-parent/Relocated.app");
    fs::create_dir_all(relocated.parent().unwrap()).unwrap();
    extract_release_archive(
        &archive.root,
        &relocated,
        &archive_spec,
        OverwritePolicy::Reject,
    )
    .expect("extract real native demo elsewhere");
    launch_to_screenshot(&relocated, &temp.0.join("relocated.png"));

    let dmg = create_dmg(
        &DmgInputs {
            app_bundle: staged.root.clone(),
            package_record: staged.package_record.clone(),
            destination: temp.0.join("EffinDOM-Native-Demo.dmg"),
            volume_name: "EffinDOM Native Demo".to_string(),
            hdiutil: PathBuf::from("/usr/bin/hdiutil"),
        },
        OverwritePolicy::Reject,
    )
    .expect("create real native demo DMG");
    launch_from_dmg(&dmg.path, staged.root.file_name().unwrap(), &temp.0);

    let inner_artifacts = fs::read_dir(staged.root.join("Contents/Frameworks"))
        .unwrap()
        .map(|entry| PathBuf::from("Contents/Frameworks").join(entry.unwrap().file_name()))
        .collect();
    let signed = sign_macos_application(
        &MacOsSigningInputs {
            unsigned_app: staged.root,
            destination: temp.0.join("signed/EffinDOM Native Demo.app"),
            ditto: PathBuf::from("/usr/bin/ditto"),
            codesign: PathBuf::from("/usr/bin/codesign"),
            identity: "-".to_string(),
            inner_artifacts,
            purpose: ArtifactSigningPurpose::LocalValidation,
            notarization: None,
        },
        OverwritePolicy::Reject,
    )
    .expect("ad-hoc sign real native demo");
    assert!(signed.record.verified);
    launch_to_screenshot(&signed.path, &temp.0.join("signed.png"));
}

fn library_outputs(paths: Vec<PathBuf>) -> Vec<NativeLibraryOutput> {
    paths
        .into_iter()
        .map(|path| NativeLibraryOutput::from_file(path).expect("native library file name"))
        .collect()
}

fn libraries(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut effindom = Vec::new();
    let mut third_party = Vec::new();
    for entry in fs::read_dir(root).expect("read native frameworks") {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if name.starts_with("libeffindom_") {
            effindom.push(path);
        } else {
            third_party.push(path);
        }
    }
    effindom.sort();
    third_party.sort();
    (effindom, third_party)
}

fn assert_macos_metadata(app: &Path) {
    let plist = fs::read_to_string(app.join("Contents/Info.plist")).expect("Info.plist");
    assert!(plist.contains("EffinDOM Native FUI-RS"));
    assert!(plist.contains("dev.effindom.native-demo"));
    assert!(plist.contains("effindom-native-fui-rs-app"));
    let icon = app.join("Contents/Resources/application.icns");
    assert!(fs::metadata(icon).unwrap().len() > 1_000);
}

fn assert_loader_paths_are_relocatable(app: &Path) {
    let executable = app.join("Contents/MacOS/effindom-native-fui-rs-app");
    let output = Command::new("/usr/bin/otool")
        .arg("-l")
        .arg(executable)
        .output()
        .expect("inspect packaged executable");
    assert!(output.status.success());
    let value = String::from_utf8_lossy(&output.stdout);
    assert!(value.contains("@loader_path/../Frameworks"));
    assert!(!value.contains("/Users/"));
}

fn launch_to_screenshot(app: &Path, screenshot: &Path) {
    let executable = app.join("Contents/MacOS/effindom-native-fui-rs-app");
    let status = Command::new(executable)
        .args(["--hidden", "--package-self-test", "--screenshot"])
        .arg(screenshot)
        .current_dir(std::env::temp_dir())
        .env_clear()
        .status()
        .expect("launch packaged native demo");
    assert!(status.success());
    let bytes = fs::read(screenshot).expect("packaged screenshot");
    assert_rendered_screenshot(&bytes);
}

fn assert_rendered_screenshot(bytes: &[u8]) {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(bytes.len() > 10_000);
    let pixels = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .expect("decode packaged screenshot")
        .to_rgba8();
    assert!(pixels.width() >= 640);
    assert!(pixels.height() >= 480);
    let mut colors = std::collections::BTreeSet::new();
    for pixel in pixels.pixels() {
        colors.insert(u32::from_be_bytes(pixel.0));
        if colors.len() >= 64 {
            break;
        }
    }
    assert!(colors.len() >= 64, "packaged screenshot is visually empty");
}

fn launch_from_dmg(dmg: &Path, app_name: &std::ffi::OsStr, root: &Path) {
    let mount = root.join("mounted-dmg");
    fs::create_dir(&mount).unwrap();
    let attached = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-quiet", "-readonly", "-nobrowse", "-mountpoint"])
        .arg(&mount)
        .arg(dmg)
        .status()
        .expect("mount real native demo DMG");
    assert!(attached.success());
    let screenshot = root.join("mounted.png");
    launch_to_screenshot(&mount.join(app_name), &screenshot);
    let detached = Command::new("/usr/bin/hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mount)
        .status()
        .expect("detach real native demo DMG");
    assert!(detached.success());
}
