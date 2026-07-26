#![cfg(unix)]

use cargo_fui::{
    sign_macos_application, sign_windows_msix, ArtifactSigningPurpose, MacOsNotarization,
    MacOsSigningInputs, OverwritePolicy, WindowsCertificateKind, WindowsSigningInputs,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "cargo-fui-signing-{name}-{}-{id}",
            std::process::id()
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

fn executable_script(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).unwrap();
    path
}

#[test]
fn windows_signing_copies_before_mutation_and_records_distinct_hashes() {
    let temp = TempDir::new("windows-copy");
    let unsigned = temp.0.join("unsigned.msix");
    let signed = temp.0.join("signed.msix");
    fs::write(&unsigned, b"unsigned-package").unwrap();
    let tool = executable_script(
        &temp.0,
        "signtool",
        r#"
case "$1" in
  sign)
    for last do :; done
    printf '\nsigned-by-test-tool\n' >> "$last"
    ;;
  verify)
    for last do :; done
    grep -q signed-by-test-tool "$last"
    ;;
  *) exit 2 ;;
esac
"#,
    );

    let artifact = sign_windows_msix(
        &WindowsSigningInputs {
            unsigned_msix: unsigned.clone(),
            destination: signed.clone(),
            signtool: tool.clone(),
            certificate_thumbprint: "A1B2C3D4".to_string(),
            certificate_kind: WindowsCertificateKind::Test,
            purpose: ArtifactSigningPurpose::LocalValidation,
            timestamp_url: None,
        },
        OverwritePolicy::Reject,
    )
    .unwrap();

    assert_eq!(fs::read(&unsigned).unwrap(), b"unsigned-package");
    assert_ne!(fs::read(&signed).unwrap(), fs::read(&unsigned).unwrap());
    assert_ne!(
        artifact.record.unsigned_sha256,
        artifact.record.signed_sha256
    );
    assert!(!artifact.record.verified);
    let record = fs::read_to_string(&artifact.signing_record).unwrap();
    assert!(!record.contains("A1B2C3D4"));
    assert!(!record.to_ascii_lowercase().contains("password"));
    assert!(artifact.checksum.exists());
}

#[test]
fn release_signing_fails_closed_before_invoking_tools() {
    let temp = TempDir::new("fail-closed");
    let unsigned = temp.0.join("unsigned.msix");
    fs::write(&unsigned, b"unsigned-package").unwrap();
    let marker = temp.0.join("tool-ran");
    let tool = executable_script(
        &temp.0,
        "must-not-run",
        &format!("touch '{}'\nexit 99", marker.display()),
    );
    let error = sign_windows_msix(
        &WindowsSigningInputs {
            unsigned_msix: unsigned,
            destination: temp.0.join("signed.msix"),
            signtool: tool.clone(),
            certificate_thumbprint: "A1B2".to_string(),
            certificate_kind: WindowsCertificateKind::Test,
            purpose: ArtifactSigningPurpose::Release,
            timestamp_url: None,
        },
        OverwritePolicy::Reject,
    )
    .unwrap_err();
    assert!(error.to_string().contains("rejects test certificates"));
    assert!(!marker.exists());

    let error = sign_windows_msix(
        &WindowsSigningInputs {
            unsigned_msix: temp.0.join("unsigned.msix"),
            destination: temp.0.join("release.msix"),
            signtool: tool,
            certificate_thumbprint: "A1B2".to_string(),
            certificate_kind: WindowsCertificateKind::Production,
            purpose: ArtifactSigningPurpose::Release,
            timestamp_url: None,
        },
        OverwritePolicy::Reject,
    )
    .unwrap_err();
    assert!(error.to_string().contains("RFC 3161 timestamp URL"));
    assert!(!marker.exists());

    let app = temp.0.join("Unsigned.app");
    fs::create_dir(&app).unwrap();
    let error = sign_macos_application(
        &MacOsSigningInputs {
            unsigned_app: app,
            destination: temp.0.join("Signed.app"),
            ditto: PathBuf::from("ditto"),
            codesign: PathBuf::from("codesign"),
            identity: "-".to_string(),
            inner_artifacts: Vec::new(),
            purpose: ArtifactSigningPurpose::Release,
            notarization: None,
        },
        OverwritePolicy::Reject,
    )
    .unwrap_err();
    assert!(error.to_string().contains("rejects ad-hoc identities"));
}

#[test]
fn macos_release_signs_inner_before_outer_then_notarizes_and_staples() {
    let temp = TempDir::new("mac-order");
    let unsigned = temp.0.join("Unsigned.app");
    let inner = unsigned.join("Contents/MacOS/worker");
    fs::create_dir_all(inner.parent().unwrap()).unwrap();
    fs::write(&inner, b"worker").unwrap();
    let log = temp.0.join("commands.log");
    let ditto = executable_script(
        &temp.0,
        "ditto",
        &format!(
            r#"
printf 'ditto %s %s\n' "$1" "${{2:-}}" >> '{}'
if [ "$1" = "-c" ]; then
  for last do :; done
  printf archive > "$last"
else
  cp -R "$1" "$2"
fi
"#,
            log.display()
        ),
    );
    let codesign = executable_script(
        &temp.0,
        "codesign",
        &format!(
            r#"
for last do :; done
printf 'codesign %s\n' "$*" >> '{}'
case " $* " in
  *" --verify "*) exit 0 ;;
esac
if [ -d "$last" ]; then
  mkdir -p "$last/Contents/_CodeSignature"
  printf signed > "$last/Contents/_CodeSignature/CodeResources"
else
  printf '\nsigned\n' >> "$last"
fi
"#,
            log.display()
        ),
    );
    let xcrun = executable_script(
        &temp.0,
        "xcrun",
        &format!("printf 'xcrun %s\\n' \"$*\" >> '{}'", log.display()),
    );

    let artifact = sign_macos_application(
        &MacOsSigningInputs {
            unsigned_app: unsigned,
            destination: temp.0.join("Signed.app"),
            ditto,
            codesign,
            identity: "Developer ID Application: Example".to_string(),
            inner_artifacts: vec![PathBuf::from("Contents/MacOS/worker")],
            purpose: ArtifactSigningPurpose::Release,
            notarization: Some(MacOsNotarization {
                xcrun,
                keychain_profile: "release-profile".to_string(),
            }),
        },
        OverwritePolicy::Reject,
    )
    .unwrap();

    let commands = fs::read_to_string(log).unwrap();
    let lines = commands.lines().collect::<Vec<_>>();
    let inner_position = lines
        .iter()
        .position(|line| line.starts_with("codesign ") && line.ends_with("Contents/MacOS/worker"))
        .unwrap();
    let outer_position = lines
        .iter()
        .position(|line| line.starts_with("codesign ") && line.ends_with(".app"))
        .unwrap();
    let submit_position = lines
        .iter()
        .position(|line| line.starts_with("xcrun notarytool"))
        .unwrap();
    let staple_position = lines
        .iter()
        .position(|line| line.starts_with("xcrun stapler"))
        .unwrap();
    assert!(inner_position < outer_position);
    assert!(outer_position < submit_position);
    assert!(submit_position < staple_position);
    assert!(commands.contains("codesign --force --options runtime --timestamp --sign"));
    assert!(commands.contains("notarytool submit"));
    assert!(commands.contains("--keychain-profile release-profile --wait"));
    assert!(artifact.record.timestamped);
    assert!(artifact.record.notarized);
    assert!(artifact.record.verified);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_adhoc_signing_uses_real_platform_tools() {
    let temp = TempDir::new("mac-real");
    let unsigned = temp.0.join("Unsigned.app");
    let executable = unsigned.join("Contents/MacOS/SigningFixture");
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    let source = temp.0.join("main.c");
    fs::write(&source, "int main(void) { return 0; }\n").unwrap();
    assert!(Command::new("/usr/bin/clang")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap()
        .success());
    fs::write(
        unsigned.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>SigningFixture</string><key>CFBundleIdentifier</key><string>dev.effindom.signing-fixture</string></dict></plist>"#,
    )
    .unwrap();

    let artifact = sign_macos_application(
        &MacOsSigningInputs {
            unsigned_app: unsigned,
            destination: temp.0.join("Signed.app"),
            ditto: PathBuf::from("/usr/bin/ditto"),
            codesign: PathBuf::from("/usr/bin/codesign"),
            identity: "-".to_string(),
            inner_artifacts: Vec::new(),
            purpose: ArtifactSigningPurpose::LocalValidation,
            notarization: None,
        },
        OverwritePolicy::Reject,
    )
    .unwrap();
    assert!(Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&artifact.path)
        .status()
        .unwrap()
        .success());
    assert!(
        Command::new(artifact.path.join("Contents/MacOS/SigningFixture"))
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn signing_rejects_input_output_overlap_without_deleting_source() {
    let temp = TempDir::new("overlap");
    let unsigned = temp.0.join("same.msix");
    fs::write(&unsigned, b"keep-me").unwrap();
    let error = sign_windows_msix(
        &WindowsSigningInputs {
            unsigned_msix: unsigned.clone(),
            destination: unsigned.clone(),
            signtool: PathBuf::from("signtool"),
            certificate_thumbprint: "A1B2".to_string(),
            certificate_kind: WindowsCertificateKind::Test,
            purpose: ArtifactSigningPurpose::LocalValidation,
            timestamp_url: None,
        },
        OverwritePolicy::Replace,
    )
    .unwrap_err();
    assert!(error.to_string().contains("must not overlap"));
    assert_eq!(fs::read(unsigned).unwrap(), b"keep-me");
}
