use crate::{
    decode_native_runtime_release_manifest, extract_native_runtime_artifact,
    verify_native_runtime_directory, Error, NativeRuntimeReleaseManifest, NativeRuntimeTarget,
    OverwritePolicy, Result,
};
use semver::Version;
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::sync::Arc;

pub const DEFAULT_NATIVE_RUNTIME_RELEASE_BASE_URL: &str =
    "https://github.com/zion-sati/EffinDOM/releases/download";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeRequirement {
    pub release: Version,
    pub core_abi: u32,
    pub ui_abi: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeRuntimeAcquisition {
    pub requirement: RuntimeRequirement,
    pub target: NativeRuntimeTarget,
    pub cache_root: PathBuf,
    pub override_root: Option<PathBuf>,
    pub offline: bool,
    pub release_base_url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeRuntimeSource {
    Override,
    Cache,
    Download,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcquiredNativeRuntime {
    pub root: PathBuf,
    pub source: NativeRuntimeSource,
    pub source_commit: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeRuntimeCacheEntry {
    pub release: Version,
    pub target: NativeRuntimeTarget,
    pub root: PathBuf,
}

pub trait RuntimeDownloader {
    fn download(&self, url: &str) -> Result<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UreqRuntimeDownloader;

impl RuntimeDownloader for UreqRuntimeDownloader {
    fn download(&self, url: &str) -> Result<Vec<u8>> {
        #[cfg(target_os = "windows")]
        let response = {
            let connector =
                ureq::native_tls::TlsConnector::new().map_err(|source| Error::RuntimeDownload {
                    url: url.to_string(),
                    message: format!("initialize Windows TLS: {source}"),
                })?;
            ureq::builder()
                .tls_connector(Arc::new(connector))
                .build()
                .get(url)
                .call()
        };
        #[cfg(not(target_os = "windows"))]
        let response = ureq::get(url).call();
        let response = response.map_err(|source| Error::RuntimeDownload {
            url: url.to_string(),
            message: source.to_string(),
        })?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|source| Error::RuntimeDownload {
                url: url.to_string(),
                message: source.to_string(),
            })?;
        Ok(bytes)
    }
}

pub fn acquire_native_runtime(
    request: &NativeRuntimeAcquisition,
    downloader: &impl RuntimeDownloader,
) -> Result<AcquiredNativeRuntime> {
    if let Some(root) = &request.override_root {
        let record = verify_native_runtime_directory(root)?;
        verify_bundle_requirement(request, record.target, record.core_abi, record.ui_abi)?;
        return Ok(AcquiredNativeRuntime {
            root: root.clone(),
            source: NativeRuntimeSource::Override,
            source_commit: record.source_commit,
        });
    }

    let release_root = request
        .cache_root
        .join(request.requirement.release.to_string());
    let manifest_path = release_root.join("native-runtime-manifest.json");
    match read_manifest_if_present(&manifest_path) {
        Ok(Some(manifest)) => match verified_cache_hit(request, &manifest, &release_root) {
            Ok(Some(acquired)) => return Ok(acquired),
            Ok(None) => {}
            Err(error) if request.offline => return Err(error),
            Err(_) => {}
        },
        Ok(None) => {}
        Err(error) if request.offline => return Err(error),
        Err(_) => {}
    }
    if request.offline {
        return Err(Error::RuntimeUnavailable(format!(
            "{} for {} is absent from verified cache {} and offline mode forbids download",
            request.requirement.release,
            request.target.as_str(),
            request.cache_root.display()
        )));
    }

    let base = request.release_base_url.trim_end_matches('/');
    let tag = format!("v{}", request.requirement.release);
    let manifest_url = format!("{base}/{tag}/native-runtime-manifest.json");
    let manifest_bytes = downloader.download(&manifest_url)?;
    let manifest = decode_native_runtime_release_manifest(&manifest_bytes)?;
    verify_release_requirement(request, &manifest)?;
    let artifact = manifest
        .artifact(request.target)
        .expect("validated release has every target");
    let archive_bytes = downloader.download(&format!("{base}/{tag}/{}", artifact.archive))?;
    let temporary = unique_temporary(&request.cache_root, "download")?;
    fs::create_dir_all(&temporary)
        .map_err(|source| runtime_io("create runtime download directory", &temporary, source))?;
    let result = (|| {
        let archive_path = temporary.join(&artifact.archive);
        fs::write(&archive_path, archive_bytes).map_err(|source| {
            runtime_io("write downloaded runtime archive", &archive_path, source)
        })?;
        let final_root = cache_bundle_root(&release_root, request.target, &artifact.archive_sha256);
        if final_root.exists() {
            fs::remove_dir_all(&final_root).map_err(|source| {
                runtime_io("replace invalid runtime cache", &final_root, source)
            })?;
        }
        if let Some(parent) = final_root.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| runtime_io("create runtime cache directory", parent, source))?;
        }
        extract_native_runtime_artifact(
            &temporary,
            &final_root,
            &manifest.source_commit,
            artifact,
            OverwritePolicy::Reject,
        )?;
        fs::write(&manifest_path, manifest_bytes).map_err(|source| {
            runtime_io("write runtime release manifest", &manifest_path, source)
        })?;
        Ok(AcquiredNativeRuntime {
            root: final_root,
            source: NativeRuntimeSource::Download,
            source_commit: manifest.source_commit.clone(),
        })
    })();
    let _ = fs::remove_dir_all(&temporary);
    result
}

pub fn list_native_runtime_cache(
    cache_root: impl AsRef<Path>,
) -> Result<Vec<NativeRuntimeCacheEntry>> {
    let cache_root = cache_root.as_ref();
    if !cache_root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for release_entry in read_directory(cache_root)? {
        let release_entry = release_entry
            .map_err(|source| runtime_io("read runtime cache entry", cache_root, source))?;
        if !release_entry.path().is_dir() {
            continue;
        }
        let Ok(release) = Version::parse(&release_entry.file_name().to_string_lossy()) else {
            continue;
        };
        let Some(manifest) =
            read_manifest_if_present(&release_entry.path().join("native-runtime-manifest.json"))?
        else {
            continue;
        };
        for artifact in manifest.artifacts {
            let root = cache_bundle_root(
                &release_entry.path(),
                artifact.target,
                &artifact.archive_sha256,
            );
            if root.is_dir() {
                if let Ok(record) = verify_native_runtime_directory(&root) {
                    if record.source_commit == manifest.source_commit
                        && record.target == artifact.target
                        && record.core_abi == artifact.core_abi
                        && record.ui_abi == artifact.ui_abi
                        && record.files == artifact.files
                    {
                        entries.push(NativeRuntimeCacheEntry {
                            release: release.clone(),
                            target: artifact.target,
                            root,
                        });
                    }
                }
            }
        }
    }
    entries.sort_by(|left, right| {
        left.release
            .cmp(&right.release)
            .then(left.target.cmp(&right.target))
    });
    Ok(entries)
}

pub fn clean_native_runtime_cache(
    cache_root: impl AsRef<Path>,
    release: Option<&Version>,
) -> Result<()> {
    let path = release.map_or_else(
        || cache_root.as_ref().to_path_buf(),
        |version| cache_root.as_ref().join(version.to_string()),
    );
    if path.exists() {
        fs::remove_dir_all(&path)
            .map_err(|source| runtime_io("clean runtime cache", &path, source))?;
    }
    Ok(())
}

pub fn runtime_requirement_from_cargo_metadata(bytes: &[u8]) -> Result<RuntimeRequirement> {
    #[derive(Deserialize)]
    struct Metadata {
        packages: Vec<Package>,
    }
    #[derive(Deserialize)]
    struct Package {
        name: String,
        metadata: serde_json::Value,
    }
    let metadata: Metadata = serde_json::from_slice(bytes)
        .map_err(|source| Error::RuntimeRequirement(source.to_string()))?;
    let package = metadata
        .packages
        .iter()
        .find(|package| package.name == "fui-rs")
        .ok_or_else(|| {
            Error::RuntimeRequirement("Cargo metadata contains no fui-rs package".to_string())
        })?;
    let effindom = package.metadata.get("effindom").ok_or_else(|| {
        Error::RuntimeRequirement("fui-rs has no package.metadata.effindom table".to_string())
    })?;
    let release = effindom
        .get("runtime-version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            Error::RuntimeRequirement("fui-rs runtime-version is missing".to_string())
        })?;
    let number = |name| {
        effindom
            .get(name)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                Error::RuntimeRequirement(format!("fui-rs {name} is missing or invalid"))
            })
    };
    Ok(RuntimeRequirement {
        release: Version::parse(release)
            .map_err(|source| Error::RuntimeRequirement(source.to_string()))?,
        core_abi: number("core-abi")?,
        ui_abi: number("ui-abi")?,
    })
}

fn verified_cache_hit(
    request: &NativeRuntimeAcquisition,
    manifest: &NativeRuntimeReleaseManifest,
    release_root: &Path,
) -> Result<Option<AcquiredNativeRuntime>> {
    verify_release_requirement(request, manifest)?;
    let artifact = manifest
        .artifact(request.target)
        .expect("validated release has every target");
    let root = cache_bundle_root(release_root, request.target, &artifact.archive_sha256);
    if !root.is_dir() {
        return Ok(None);
    }
    let record = verify_native_runtime_directory(&root)?;
    if record.source_commit != manifest.source_commit || record.files != artifact.files {
        return Ok(None);
    }
    verify_bundle_requirement(request, record.target, record.core_abi, record.ui_abi)?;
    Ok(Some(AcquiredNativeRuntime {
        root,
        source: NativeRuntimeSource::Cache,
        source_commit: manifest.source_commit.clone(),
    }))
}

fn verify_release_requirement(
    request: &NativeRuntimeAcquisition,
    manifest: &NativeRuntimeReleaseManifest,
) -> Result<()> {
    if manifest.release != request.requirement.release.to_string() {
        return Err(Error::RuntimeRequirement(format!(
            "release manifest is {}, expected {}",
            manifest.release, request.requirement.release
        )));
    }
    let artifact = manifest
        .artifact(request.target)
        .expect("validated release has every target");
    verify_bundle_requirement(request, artifact.target, artifact.core_abi, artifact.ui_abi)
}

fn verify_bundle_requirement(
    request: &NativeRuntimeAcquisition,
    target: NativeRuntimeTarget,
    core_abi: u32,
    ui_abi: u32,
) -> Result<()> {
    if target != request.target
        || core_abi != request.requirement.core_abi
        || ui_abi != request.requirement.ui_abi
    {
        return Err(Error::RuntimeRequirement(format!(
            "runtime target/ABI is {}/{core_abi}/{ui_abi}, expected {}/{}/{}",
            target.as_str(),
            request.target.as_str(),
            request.requirement.core_abi,
            request.requirement.ui_abi
        )));
    }
    Ok(())
}

fn read_manifest_if_present(path: &Path) -> Result<Option<NativeRuntimeReleaseManifest>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(decode_native_runtime_release_manifest(
        &fs::read(path)
            .map_err(|source| runtime_io("read cached runtime manifest", path, source))?,
    )?))
}

fn cache_bundle_root(release_root: &Path, target: NativeRuntimeTarget, hash: &str) -> PathBuf {
    release_root.join(target.as_str()).join(hash)
}

fn read_directory(path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path).map_err(|source| runtime_io("read runtime cache", path, source))
}

fn unique_temporary(parent: &Path, label: &str) -> Result<PathBuf> {
    fs::create_dir_all(parent)
        .map_err(|source| runtime_io("create runtime cache root", parent, source))?;
    for suffix in 0..1024_u32 {
        let path = parent.join(format!(".{label}-{}-{suffix}", std::process::id()));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(Error::RuntimeUnavailable(
        "could not reserve a temporary runtime cache path".to_string(),
    ))
}

fn runtime_io(operation: &'static str, path: &Path, source: std::io::Error) -> Error {
    Error::RuntimeIo {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
