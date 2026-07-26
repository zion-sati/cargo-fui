use crate::{Error, OverwritePolicy, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const SIGNING_RECORD_SCHEMA_VERSION: u32 = 1;
static NEXT_SIGNING_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactSigningPurpose {
    LocalValidation,
    Release,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WindowsCertificateKind {
    Test,
    Production,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsSigningInputs {
    pub unsigned_msix: PathBuf,
    pub destination: PathBuf,
    pub signtool: PathBuf,
    pub certificate_thumbprint: String,
    pub certificate_kind: WindowsCertificateKind,
    pub purpose: ArtifactSigningPurpose,
    pub timestamp_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacOsNotarization {
    pub xcrun: PathBuf,
    pub keychain_profile: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacOsSigningInputs {
    pub unsigned_app: PathBuf,
    pub destination: PathBuf,
    pub ditto: PathBuf,
    pub codesign: PathBuf,
    pub identity: String,
    pub inner_artifacts: Vec<PathBuf>,
    pub purpose: ArtifactSigningPurpose,
    pub notarization: Option<MacOsNotarization>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignedArtifactRecord {
    pub schema_version: u32,
    pub platform: String,
    pub purpose: ArtifactSigningPurpose,
    pub unsigned_sha256: String,
    pub signed_sha256: String,
    pub timestamped: bool,
    pub notarized: bool,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedArtifact {
    pub path: PathBuf,
    pub checksum: PathBuf,
    pub signing_record: PathBuf,
    pub record: SignedArtifactRecord,
}

pub fn sign_windows_msix(
    inputs: &WindowsSigningInputs,
    overwrite: OverwritePolicy,
) -> Result<SignedArtifact> {
    validate_windows(inputs)?;
    let source = canonical_file(&inputs.unsigned_msix, "resolve unsigned MSIX")?;
    let unsigned_sha256 = hash_file(&source)?;
    let destination = prepare_destination(&inputs.destination, overwrite, false)?;
    reject_overlapping_paths(&source, &destination)?;
    let temporary = unique_file_sibling(&destination, "signed-msix")?;
    fs::copy(&source, &temporary)
        .map_err(|source| signing_io("copy unsigned MSIX", &temporary, source))?;
    let result = (|| {
        let mut sign = Command::new(&inputs.signtool);
        sign.args(["sign", "/fd", "SHA256", "/s", "My", "/sha1"])
            .arg(&inputs.certificate_thumbprint);
        if let Some(timestamp_url) = &inputs.timestamp_url {
            sign.args(["/td", "SHA256", "/tr"]).arg(timestamp_url);
        }
        sign.arg(&temporary);
        run_tool("signtool", "sign MSIX", &temporary, &mut sign)?;
        if inputs.purpose == ArtifactSigningPurpose::Release {
            run_tool(
                "signtool",
                "verify signed MSIX",
                &temporary,
                Command::new(&inputs.signtool)
                    .args(["verify", "/pa", "/v"])
                    .arg(&temporary),
            )?;
        }
        publish_file(&temporary, &destination, overwrite)?;
        finalize_signed_artifact(
            destination,
            SignedArtifactRecord {
                schema_version: SIGNING_RECORD_SCHEMA_VERSION,
                platform: "windows".to_string(),
                purpose: inputs.purpose,
                unsigned_sha256,
                signed_sha256: String::new(),
                timestamped: inputs.timestamp_url.is_some(),
                notarized: false,
                verified: inputs.purpose == ArtifactSigningPurpose::Release,
            },
        )
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub fn sign_macos_application(
    inputs: &MacOsSigningInputs,
    overwrite: OverwritePolicy,
) -> Result<SignedArtifact> {
    validate_macos(inputs)?;
    let source = canonical_directory(&inputs.unsigned_app, "resolve unsigned macOS app")?;
    let unsigned_sha256 = hash_tree(&source)?;
    let destination = prepare_destination(&inputs.destination, overwrite, true)?;
    reject_overlapping_paths(&source, &destination)?;
    let temporary = unique_directory_sibling(&destination, "signed-app")?;
    let result = (|| {
        run_tool(
            "ditto",
            "copy unsigned macOS app",
            &source,
            Command::new(&inputs.ditto).arg(&source).arg(&temporary),
        )?;
        for relative in &inputs.inner_artifacts {
            let inner = resolve_inner_signing_path(&temporary, relative)?;
            codesign(inputs, &inner)?;
        }
        codesign(inputs, &temporary)?;
        run_tool(
            "codesign",
            "verify signed macOS app",
            &temporary,
            Command::new(&inputs.codesign)
                .args(["--verify", "--deep", "--strict", "--verbose=2"])
                .arg(&temporary),
        )?;
        if let Some(notarization) = &inputs.notarization {
            let archive = unique_file_with_extension(&destination, "notarization", "zip")?;
            run_tool(
                "ditto",
                "archive signed app for notarization",
                &temporary,
                Command::new(&inputs.ditto)
                    .args(["-c", "-k", "--keepParent"])
                    .arg(&temporary)
                    .arg(&archive),
            )?;
            let submit = run_tool(
                "xcrun",
                "submit signed app for notarization",
                &archive,
                Command::new(&notarization.xcrun)
                    .args(["notarytool", "submit"])
                    .arg(&archive)
                    .args([
                        "--keychain-profile",
                        &notarization.keychain_profile,
                        "--wait",
                    ]),
            );
            let _ = fs::remove_file(&archive);
            submit?;
            run_tool(
                "xcrun",
                "staple notarization ticket",
                &temporary,
                Command::new(&notarization.xcrun)
                    .args(["stapler", "staple"])
                    .arg(&temporary),
            )?;
        }
        publish_directory(&temporary, &destination, overwrite)?;
        finalize_signed_artifact(
            destination,
            SignedArtifactRecord {
                schema_version: SIGNING_RECORD_SCHEMA_VERSION,
                platform: "macos".to_string(),
                purpose: inputs.purpose,
                unsigned_sha256,
                signed_sha256: String::new(),
                timestamped: inputs.purpose == ArtifactSigningPurpose::Release,
                notarized: inputs.notarization.is_some(),
                verified: true,
            },
        )
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(temporary);
    }
    result
}

fn validate_windows(inputs: &WindowsSigningInputs) -> Result<()> {
    if inputs.certificate_thumbprint.is_empty()
        || !inputs
            .certificate_thumbprint
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(configuration(
            "Windows certificate thumbprint must contain only hexadecimal digits",
        ));
    }
    if inputs.purpose == ArtifactSigningPurpose::Release {
        if inputs.certificate_kind != WindowsCertificateKind::Production {
            return Err(configuration(
                "release MSIX signing rejects test certificates",
            ));
        }
        if inputs.timestamp_url.as_deref().is_none_or(str::is_empty) {
            return Err(configuration(
                "release MSIX signing requires an RFC 3161 timestamp URL",
            ));
        }
    }
    Ok(())
}

fn validate_macos(inputs: &MacOsSigningInputs) -> Result<()> {
    if inputs.identity.is_empty() {
        return Err(configuration("macOS signing identity must not be empty"));
    }
    if inputs.purpose == ArtifactSigningPurpose::Release {
        if inputs.identity == "-" || inputs.identity.eq_ignore_ascii_case("adhoc") {
            return Err(configuration(
                "release macOS signing rejects ad-hoc identities",
            ));
        }
        let Some(notarization) = &inputs.notarization else {
            return Err(configuration(
                "release macOS signing requires notarization configuration",
            ));
        };
        if notarization.keychain_profile.is_empty() {
            return Err(configuration(
                "macOS notarization keychain profile must not be empty",
            ));
        }
    } else if inputs.notarization.is_some() {
        return Err(configuration(
            "local macOS validation cannot request release notarization",
        ));
    }
    Ok(())
}

fn codesign(inputs: &MacOsSigningInputs, path: &Path) -> Result<()> {
    let mut command = Command::new(&inputs.codesign);
    command.arg("--force");
    if inputs.purpose == ArtifactSigningPurpose::Release {
        command.args(["--options", "runtime", "--timestamp"]);
    } else {
        command.arg("--timestamp=none");
    }
    command.args(["--sign", &inputs.identity]).arg(path);
    run_tool("codesign", "sign macOS artifact", path, &mut command).map(|_| ())
}

fn finalize_signed_artifact(
    path: PathBuf,
    mut record: SignedArtifactRecord,
) -> Result<SignedArtifact> {
    record.signed_sha256 = if path.is_dir() {
        hash_tree(&path)?
    } else {
        hash_file(&path)?
    };
    if record.signed_sha256 == record.unsigned_sha256 {
        return Err(configuration(
            "signing completed without changing the artifact digest",
        ));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| configuration("signed artifact must have a UTF-8 file name"))?;
    let checksum = path.with_file_name(format!("{name}.sha256"));
    fs::write(&checksum, format!("{}  {name}\n", record.signed_sha256))
        .map_err(|source| signing_io("write signed-artifact checksum", &checksum, source))?;
    let signing_record = path.with_file_name(format!("{name}.effindom-signing.json"));
    let value = serde_json::to_vec_pretty(&record).map_err(Error::SerializeSigningRecord)?;
    fs::write(&signing_record, value)
        .map_err(|source| signing_io("write signed-artifact record", &signing_record, source))?;
    Ok(SignedArtifact {
        path,
        checksum,
        signing_record,
        record,
    })
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).map_err(|source| signing_io("open artifact for hashing", path, source))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| signing_io("hash artifact", path, source))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex_digest(digest.finalize()))
}

fn hash_tree(root: &Path) -> Result<String> {
    let mut entries = Vec::new();
    collect_tree(root, root, &mut entries)?;
    entries.sort();
    let mut digest = Sha256::new();
    for relative in entries {
        let path = root.join(&relative);
        digest.update(relative.to_string_lossy().as_bytes());
        digest.update([0]);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| signing_io("inspect signed bundle", &path, source))?;
        if metadata.file_type().is_symlink() {
            digest.update(b"link\0");
            digest.update(
                fs::read_link(&path)
                    .map_err(|source| signing_io("read signed bundle link", &path, source))?
                    .to_string_lossy()
                    .as_bytes(),
            );
        } else if metadata.is_file() {
            digest.update(b"file\0");
            let mut file = File::open(&path)
                .map_err(|source| signing_io("open signed bundle file", &path, source))?;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let count = file
                    .read(&mut buffer)
                    .map_err(|source| signing_io("hash signed bundle file", &path, source))?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
        }
        digest.update([0xff]);
    }
    Ok(hex_digest(digest.finalize()))
}

fn collect_tree(root: &Path, directory: &Path, entries: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)
        .map_err(|source| signing_io("read signed bundle", directory, source))?
    {
        let entry =
            entry.map_err(|source| signing_io("read signed bundle entry", directory, source))?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("tree entry remains below root")
            .to_path_buf();
        entries.push(relative);
        if fs::symlink_metadata(&path)
            .map_err(|source| signing_io("inspect signed bundle entry", &path, source))?
            .is_dir()
        {
            collect_tree(root, &path, entries)?;
        }
    }
    Ok(())
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn canonical_file(path: &Path, operation: &'static str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).map_err(|source| signing_io(operation, path, source))?;
    if !path.is_file() {
        return Err(configuration(
            "unsigned signing input must be a regular file",
        ));
    }
    Ok(path)
}

fn canonical_directory(path: &Path, operation: &'static str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).map_err(|source| signing_io(operation, path, source))?;
    if !path.is_dir() {
        return Err(configuration("unsigned signing input must be a directory"));
    }
    Ok(path)
}

fn prepare_destination(
    path: &Path,
    overwrite: OverwritePolicy,
    directory: bool,
) -> Result<PathBuf> {
    let path = absolute_destination(path)?;
    if path.exists() {
        if overwrite == OverwritePolicy::Reject {
            return Err(configuration("signed artifact destination already exists"));
        }
        if path.is_dir() != directory {
            return Err(configuration(
                "signed artifact destination has the wrong file type",
            ));
        }
    }
    Ok(path)
}

fn absolute_destination(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| configuration("signed artifact destination must have a file name"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|source| signing_io("create signing output parent", parent, source))?;
    Ok(fs::canonicalize(parent)
        .map_err(|source| signing_io("resolve signing output parent", parent, source))?
        .join(name))
}

fn unique_file_sibling(destination: &Path, kind: &str) -> Result<PathBuf> {
    let extension = destination.extension().and_then(|value| value.to_str());
    unique_sibling(destination, kind, extension)
}

fn unique_file_with_extension(destination: &Path, kind: &str, extension: &str) -> Result<PathBuf> {
    unique_sibling(destination, kind, Some(extension))
}

fn unique_directory_sibling(destination: &Path, kind: &str) -> Result<PathBuf> {
    let extension = destination.extension().and_then(|value| value.to_str());
    unique_sibling(destination, kind, extension)
}

fn unique_sibling(destination: &Path, kind: &str, extension: Option<&str>) -> Result<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| configuration("signing destination must have a parent"))?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| configuration("signing destination must have a UTF-8 name"))?;
    loop {
        let id = NEXT_SIGNING_ID.fetch_add(1, Ordering::Relaxed);
        let suffix = extension.map_or_else(String::new, |value| format!(".{value}"));
        let candidate = parent.join(format!(
            ".{name}.effindom-{kind}-{}-{id}{suffix}",
            std::process::id()
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
}

fn publish_file(staging: &Path, destination: &Path, overwrite: OverwritePolicy) -> Result<()> {
    if destination.exists() && overwrite == OverwritePolicy::Replace {
        fs::remove_file(destination)
            .map_err(|source| signing_io("remove replaced signed artifact", destination, source))?;
    }
    fs::rename(staging, destination)
        .map_err(|source| signing_io("publish signed artifact", destination, source))
}

fn publish_directory(staging: &Path, destination: &Path, overwrite: OverwritePolicy) -> Result<()> {
    if destination.exists() && overwrite == OverwritePolicy::Replace {
        fs::remove_dir_all(destination).map_err(|source| {
            signing_io("remove replaced signed application", destination, source)
        })?;
    }
    fs::rename(staging, destination)
        .map_err(|source| signing_io("publish signed application", destination, source))
}

fn validate_relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(configuration(
            "inner macOS signing paths must be normal relative paths",
        ));
    }
    Ok(())
}

fn resolve_inner_signing_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    validate_relative(relative)?;
    let root = fs::canonicalize(root)
        .map_err(|source| signing_io("resolve staged macOS application", root, source))?;
    let candidate = root.join(relative);
    let resolved = fs::canonicalize(&candidate)
        .map_err(|source| signing_io("resolve inner macOS signing artifact", &candidate, source))?;
    if !resolved.starts_with(&root) {
        return Err(configuration(
            "inner macOS signing path escapes the application bundle",
        ));
    }
    Ok(resolved)
}

fn reject_overlapping_paths(source: &Path, destination: &Path) -> Result<()> {
    if source == destination || (source.is_dir() && destination.starts_with(source)) {
        return Err(configuration(
            "signed artifact destination must not overlap its unsigned input",
        ));
    }
    Ok(())
}

fn run_tool(
    tool: &'static str,
    operation: &'static str,
    path: &Path,
    command: &mut Command,
) -> Result<std::process::Output> {
    let output = command.output().map_err(|source| Error::SigningTool {
        tool,
        operation,
        path: path.to_path_buf(),
        message: source.to_string(),
    })?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(Error::SigningTool {
            tool,
            operation,
            path: path.to_path_buf(),
            message: if stderr.is_empty() { stdout } else { stderr },
        })
    }
}

fn configuration(message: impl Into<String>) -> Error {
    Error::SigningConfiguration(message.into())
}

fn signing_io(operation: &'static str, path: &Path, source: std::io::Error) -> Error {
    Error::SigningIo {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
