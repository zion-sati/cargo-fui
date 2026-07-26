#![cfg(target_os = "linux")]

use cargo_fui::{
    create_appimage, create_release_archive, extract_release_archive, load_icon_source,
    resolve_package_contract, stage_native_bundle, AppImageInputs, BuildProfile, NativeBuildOutput,
    NativeLibraryOutput, OverwritePolicy, PackageRequest, ReleaseArchiveFormat, ReleaseArchiveSpec,
    SigningMode,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn packages_relocates_and_launches_the_real_native_demo() {
    let Some(output_root) = std::env::var_os("EFFINDOM_NATIVE_DEMO_OUTPUT") else {
        eprintln!("skipped: set EFFINDOM_NATIVE_DEMO_OUTPUT for real Linux package acceptance");
        return;
    };
    let Some(manifest) = std::env::var_os("EFFINDOM_NATIVE_DEMO_MANIFEST") else {
        eprintln!("skipped: set EFFINDOM_NATIVE_DEMO_MANIFEST for real Linux package acceptance");
        return;
    };
    let Some(appimagetool) = std::env::var_os("EFFINDOM_APPIMAGETOOL") else {
        eprintln!("skipped: set EFFINDOM_APPIMAGETOOL for real Linux package acceptance");
        return;
    };
    let Some(unsquashfs) = std::env::var_os("EFFINDOM_UNSQUASHFS") else {
        eprintln!("skipped: set EFFINDOM_UNSQUASHFS for real Linux package acceptance");
        return;
    };
    let unsquashfs = PathBuf::from(unsquashfs);
    let Some(package_root) = std::env::var_os("EFFINDOM_NATIVE_DEMO_PACKAGE_OUTPUT") else {
        eprintln!(
            "skipped: set EFFINDOM_NATIVE_DEMO_PACKAGE_OUTPUT for real Linux package acceptance"
        );
        return;
    };

    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    let output_root = PathBuf::from(output_root);
    let package_root = PathBuf::from(package_root);
    if package_root.exists() {
        fs::remove_dir_all(&package_root).expect("remove prior real-demo package output");
    }
    fs::create_dir_all(&package_root).expect("create real-demo package output");

    let contract = resolve_package_contract(
        PathBuf::from(manifest),
        PackageRequest::new(target, BuildProfile::Release, SigningMode::Unsigned),
    )
    .expect("resolve real native demo package contract");
    let (effindom_libraries, third_party_libraries) = libraries(&output_root.join("lib"));
    let staged = stage_native_bundle(
        &contract,
        &NativeBuildOutput {
            application_executable: output_root.join("bin/effindom_v2_linux_native"),
            effindom_runtime_libraries: effindom_libraries,
            third_party_libraries,
            runtime_resources: output_root.join("share/effindom"),
            application_resources: output_root.join("share/app"),
        },
        package_root.join("staged"),
        OverwritePolicy::Reject,
    )
    .expect("stage real native demo");
    assert!(staged
        .root
        .join("lib/libdecor/plugins-1/libdecor-cairo.so")
        .is_file());
    launch_to_screenshot(&staged.root, &contract, &package_root.join("staged.png"));

    let archive_spec = ReleaseArchiveSpec {
        archive_name: "effindom-native-demo.tar.zst".to_string(),
        format: ReleaseArchiveFormat::TarZstd,
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
    let appimage = create_appimage(
        &AppImageInputs {
            bundle_root: staged.root,
            package_record: staged.package_record,
            destination: package_root.join("EffinDOM-Native-Demo.AppImage"),
            executable,
            metadata: contract.native_metadata().expect("native metadata"),
            categories: contract.platform_settings.linux.categories.clone(),
            icons,
            appimagetool: PathBuf::from(appimagetool),
            unsquashfs: unsquashfs.clone(),
        },
        OverwritePolicy::Reject,
    )
    .expect("create real native demo AppImage");
    assert_ne!(
        fs::metadata(&appimage.path).unwrap().permissions().mode() & 0o111,
        0
    );

    let extracted_parent = package_root.join("appimage-extracted");
    fs::create_dir(&extracted_parent).unwrap();
    let extracted = extracted_parent.join("squashfs-root");
    let status = Command::new(&unsquashfs)
        .args(["-f", "-d"])
        .arg(&extracted)
        .arg("-o")
        .arg(squashfs_offset(&appimage.path).to_string())
        .arg(&appimage.path)
        .status()
        .expect("extract AppImage without FUSE");
    assert!(status.success());
    launch_app_run(
        &extracted.join("AppRun"),
        &package_root.join("appimage.png"),
    );
}

fn libraries(root: &Path) -> (Vec<NativeLibraryOutput>, Vec<NativeLibraryOutput>) {
    let mut effindom = Vec::new();
    let mut third_party = Vec::new();
    collect_libraries(root, root, &mut effindom, &mut third_party);
    effindom.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    third_party.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    (effindom, third_party)
}

fn collect_libraries(
    root: &Path,
    directory: &Path,
    effindom: &mut Vec<NativeLibraryOutput>,
    third_party: &mut Vec<NativeLibraryOutput>,
) {
    for entry in fs::read_dir(directory).expect("read native Linux library directory") {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_libraries(root, &path, effindom, third_party);
            continue;
        }
        let relative = path.strip_prefix(root).unwrap().to_path_buf();
        let output = NativeLibraryOutput::new(&path, relative);
        if path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("libeffindom_")
        {
            effindom.push(output);
        } else {
            third_party.push(output);
        }
    }
}

fn launch_to_screenshot(root: &Path, contract: &cargo_fui::PackageContract, screenshot: &Path) {
    let executable = root.join(
        contract
            .layout
            .executable
            .strip_prefix(&contract.layout.root)
            .expect("package-relative executable"),
    );
    launch(&executable, screenshot);
}

fn launch_app_run(app_run: &Path, screenshot: &Path) {
    launch(app_run, screenshot);
}

fn launch(executable: &Path, screenshot: &Path) {
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

fn squashfs_offset(appimage: &Path) -> usize {
    fs::read(appimage)
        .expect("read generated AppImage")
        .windows(4)
        .rposition(|window| window == b"hsqs")
        .expect("generated AppImage contains SquashFS")
}
