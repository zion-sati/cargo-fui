#![cfg(target_os = "windows")]

use cargo_fui::{
    create_msix, sign_windows_msix, ApplicationMetadata, ArtifactSigningPurpose, BundleFile,
    IconAlpha, IconRaster, IconRasterSet, IconSourceFormat, MsixInputs, NativeVersion,
    OverwritePolicy, PackageArchitecture, PackageBuildMode, PackageMetadata,
    PackageOperatingSystem, PackagingInputs, WindowsCertificateKind, WindowsSigningInputs,
};
use effindom_native_packaging::stage_bundle;
use std::fs;
use std::path::PathBuf;

#[test]
fn signs_real_msix_with_ephemeral_current_user_certificate() {
    let Some(makeappx) = std::env::var_os("EFFINDOM_MAKEAPPX") else {
        eprintln!("skipped: set EFFINDOM_MAKEAPPX for native MSIX signing acceptance");
        return;
    };
    let Some(signtool) = std::env::var_os("EFFINDOM_SIGNTOOL") else {
        eprintln!("skipped: set EFFINDOM_SIGNTOOL for native MSIX signing acceptance");
        return;
    };
    let Some(thumbprint) = std::env::var_os("EFFINDOM_TEST_CERT_THUMBPRINT") else {
        eprintln!("skipped: set EFFINDOM_TEST_CERT_THUMBPRINT for native MSIX signing acceptance");
        return;
    };
    let Some(output_root) = std::env::var_os("EFFINDOM_SIGNING_TEST_OUTPUT") else {
        eprintln!("skipped: set EFFINDOM_SIGNING_TEST_OUTPUT for native MSIX signing acceptance");
        return;
    };
    let architecture_name =
        std::env::var("EFFINDOM_TEST_PACKAGE_ARCHITECTURE").unwrap_or_else(|_| "x64".to_string());
    let (architecture, target_triple) = match architecture_name.as_str() {
        "arm64" => (PackageArchitecture::Arm64, "aarch64-pc-windows-msvc"),
        "x64" => (PackageArchitecture::X64, "x86_64-pc-windows-msvc"),
        value => panic!("unsupported test package architecture: {value}"),
    };
    let root = PathBuf::from(output_root);
    fs::create_dir_all(&root).expect("create signing fixture root");
    let windows = PathBuf::from(std::env::var_os("WINDIR").expect("WINDIR"));
    let resource = root.join("resource.txt");
    fs::write(&resource, "signed package fixture").expect("write resource");
    let package_record = PathBuf::from("effindom-package.json");
    let bundle = root.join("bundle");
    stage_bundle(
        &PackagingInputs {
            destination: bundle.clone(),
            package_record: package_record.clone(),
            metadata: PackageMetadata {
                application_identifier: "dev.effindom.signingfixture".to_string(),
                application_version: "1.0.0".to_string(),
                operating_system: PackageOperatingSystem::Windows,
                architecture,
                target_triple: target_triple.to_string(),
                build_mode: PackageBuildMode::Release,
                core_abi: 2,
                ui_abi: 1,
            },
            application_executable: BundleFile::new(
                windows.join("System32/where.exe"),
                "signing-fixture.exe",
            ),
            effindom_runtime_libraries: vec![],
            third_party_libraries: vec![],
            runtime_resources: vec![],
            application_resources: vec![BundleFile::new(resource, "resource.txt")],
            metadata_artifacts: vec![],
        },
        OverwritePolicy::Reject,
    )
    .expect("stage signed Windows bundle");
    let unsigned = create_msix(
        &MsixInputs {
            bundle_root: bundle,
            package_record,
            destination: root.join("Unsigned.msix"),
            executable: PathBuf::from("signing-fixture.exe"),
            metadata: ApplicationMetadata::new(
                "signing-fixture",
                "1.0.0",
                NativeVersion::new(1, 0, 0, 0),
                "EffinDOM Signing Fixture",
                "dev.effindom.signingfixture",
            )
            .expect("metadata"),
            publisher: "CN=EffinDOM Packaging Test".to_string(),
            publisher_display_name: "EffinDOM".to_string(),
            icons: opaque_icons(),
            makeappx: PathBuf::from(makeappx),
        },
        OverwritePolicy::Reject,
    )
    .expect("create unsigned MSIX");
    let signed = sign_windows_msix(
        &WindowsSigningInputs {
            unsigned_msix: unsigned.path.clone(),
            destination: root.join("Signed.msix"),
            signtool: PathBuf::from(signtool),
            certificate_thumbprint: thumbprint.to_string_lossy().into_owned(),
            certificate_kind: WindowsCertificateKind::Test,
            purpose: ArtifactSigningPurpose::LocalValidation,
            timestamp_url: None,
        },
        OverwritePolicy::Reject,
    )
    .expect("sign MSIX");

    assert!(unsigned.path.is_file());
    assert!(signed.path.is_file());
    assert_ne!(signed.record.unsigned_sha256, signed.record.signed_sha256);
    let record = fs::read_to_string(&signed.signing_record).expect("read signing record");
    assert!(!record.contains(&thumbprint.to_string_lossy().to_string()));
}

fn opaque_icons() -> IconRasterSet {
    IconRasterSet {
        source_format: IconSourceFormat::Png,
        source_width: 256,
        source_height: 256,
        alpha: IconAlpha::Opaque,
        rasters: [44, 50, 150]
            .into_iter()
            .map(|size| IconRaster {
                size,
                rgba8: vec![255; size as usize * size as usize * 4],
            })
            .collect(),
    }
}
