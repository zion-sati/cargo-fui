#![cfg(target_os = "windows")]

use cargo_fui::{
    create_msix, create_release_archive, extract_release_archive, load_icon_source,
    resolve_package_contract, sign_windows_msix, stage_native_bundle, ArtifactSigningPurpose,
    BuildProfile, MsixInputs, NativeBuildOutput, NativeLibraryOutput, OverwritePolicy,
    PackageRequest, ReleaseArchiveFormat, ReleaseArchiveSpec, SigningMode, WindowsCertificateKind,
    WindowsSigningInputs,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn packages_relocates_signs_and_launches_the_real_native_demo() {
    let Some(output_root) = std::env::var_os("EFFINDOM_NATIVE_DEMO_OUTPUT") else {
        eprintln!("skipped: set EFFINDOM_NATIVE_DEMO_OUTPUT for real Windows package acceptance");
        return;
    };
    let Some(manifest) = std::env::var_os("EFFINDOM_NATIVE_DEMO_MANIFEST") else {
        eprintln!("skipped: set EFFINDOM_NATIVE_DEMO_MANIFEST for real Windows package acceptance");
        return;
    };
    let Some(makeappx) = std::env::var_os("EFFINDOM_MAKEAPPX") else {
        eprintln!("skipped: set EFFINDOM_MAKEAPPX for real Windows package acceptance");
        return;
    };
    let Some(signtool) = std::env::var_os("EFFINDOM_SIGNTOOL") else {
        eprintln!("skipped: set EFFINDOM_SIGNTOOL for real Windows package acceptance");
        return;
    };
    let Some(thumbprint) = std::env::var_os("EFFINDOM_TEST_CERT_THUMBPRINT") else {
        eprintln!("skipped: set EFFINDOM_TEST_CERT_THUMBPRINT for real Windows package acceptance");
        return;
    };
    let Some(package_root) = std::env::var_os("EFFINDOM_NATIVE_DEMO_PACKAGE_OUTPUT") else {
        eprintln!(
            "skipped: set EFFINDOM_NATIVE_DEMO_PACKAGE_OUTPUT for real Windows package acceptance"
        );
        return;
    };

    let architecture =
        std::env::var("EFFINDOM_TEST_PACKAGE_ARCHITECTURE").unwrap_or_else(|_| "x64".into());
    let target = match architecture.as_str() {
        "arm64" => "aarch64-pc-windows-msvc",
        "x64" => "x86_64-pc-windows-msvc",
        value => panic!("unsupported test package architecture: {value}"),
    };
    let output_root = PathBuf::from(output_root);
    let package_root = PathBuf::from(package_root);
    if package_root.exists() {
        fs::remove_dir_all(&package_root).expect("remove prior real-demo package output");
    }
    fs::create_dir_all(&package_root).expect("create real-demo package output");

    let contract = resolve_package_contract(
        PathBuf::from(manifest),
        PackageRequest::new(target, BuildProfile::Release, SigningMode::Signed),
    )
    .expect("resolve real native demo package contract");
    let (effindom_libraries, third_party_libraries) = libraries(&output_root);
    let staged = stage_native_bundle(
        &contract,
        &NativeBuildOutput {
            application_executable: output_root.join("effindom_v2_windows_native.exe"),
            effindom_runtime_libraries: library_outputs(effindom_libraries),
            third_party_libraries: library_outputs(third_party_libraries),
            runtime_resources: output_root.join("assets/effindom"),
            application_resources: output_root.join("assets/app"),
        },
        package_root.join("staged"),
        OverwritePolicy::Reject,
    )
    .expect("stage real native demo");
    launch_to_screenshot(&staged.root, &contract, &package_root.join("staged.png"));

    let archive_spec = ReleaseArchiveSpec {
        archive_name: "effindom-native-demo.zip".to_string(),
        format: ReleaseArchiveFormat::Zip,
        package_record: staged.package_record.clone(),
    };
    let archive = create_release_archive(
        &staged.root,
        package_root.join("archive"),
        &archive_spec,
        OverwritePolicy::Reject,
    )
    .expect("archive real native demo");
    let relocated = package_root.join("unrelated-parent/relocated");
    fs::create_dir_all(relocated.parent().unwrap()).unwrap();
    extract_release_archive(
        &archive.root,
        &relocated,
        &archive_spec,
        OverwritePolicy::Reject,
    )
    .expect("extract real native demo elsewhere");
    launch_to_screenshot(&relocated, &contract, &package_root.join("relocated.png"));

    let source_icon = contract
        .application
        .source_icon
        .as_ref()
        .expect("resolved source icon");
    let icons = load_icon_source(source_icon)
        .expect("load real native demo icon")
        .canonical_rasters()
        .expect("rasterize real native demo icon");
    let executable = contract
        .layout
        .executable
        .strip_prefix(&contract.layout.root)
        .expect("package-relative executable")
        .to_path_buf();
    let unsigned = create_msix(
        &MsixInputs {
            bundle_root: staged.root,
            package_record: staged.package_record,
            destination: package_root.join("EffinDOM-Native-Demo-Unsigned.msix"),
            executable,
            metadata: contract.native_metadata().expect("native metadata"),
            publisher: contract
                .platform_settings
                .windows
                .publisher
                .clone()
                .expect("resolved Windows publisher"),
            publisher_display_name: "EffinDOM".to_string(),
            icons,
            makeappx: PathBuf::from(makeappx),
        },
        OverwritePolicy::Reject,
    )
    .expect("create real native demo MSIX");
    let signed = sign_windows_msix(
        &WindowsSigningInputs {
            unsigned_msix: unsigned.path,
            destination: package_root.join("EffinDOM-Native-Demo.msix"),
            signtool: PathBuf::from(signtool),
            certificate_thumbprint: thumbprint.to_string_lossy().into_owned(),
            certificate_kind: WindowsCertificateKind::Test,
            purpose: ArtifactSigningPurpose::LocalValidation,
            timestamp_url: None,
        },
        OverwritePolicy::Reject,
    )
    .expect("sign real native demo MSIX");
    assert!(signed.path.is_file());
    assert!(signed.signing_record.is_file());
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
    for entry in fs::read_dir(root).expect("read native Windows output") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("dll") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy();
        if name.starts_with("effindom_v2_") {
            effindom.push(path);
        } else {
            third_party.push(path);
        }
    }
    effindom.sort();
    third_party.sort();
    (effindom, third_party)
}

fn launch_to_screenshot(root: &Path, contract: &cargo_fui::PackageContract, screenshot: &Path) {
    let executable = root.join(
        contract
            .layout
            .executable
            .strip_prefix(&contract.layout.root)
            .expect("package-relative executable"),
    );
    let status = Command::new(executable)
        .args(["--hidden", "--package-self-test", "--screenshot"])
        .arg(screenshot)
        .current_dir(std::env::temp_dir())
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
