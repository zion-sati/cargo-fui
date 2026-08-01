use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const FUI_MANIFEST_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    FUI_MANIFEST_SCHEMA_VERSION
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FuiManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub application: ApplicationManifest,
    #[serde(default)]
    pub assets: AssetsManifest,
    #[serde(default)]
    pub package: PackageSettings,
    #[serde(default)]
    pub workers: Vec<WorkerBundleManifest>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ApplicationManifest {
    pub identifier: String,
    pub caption: Option<String>,
    pub icon: Option<PathBuf>,
    pub cargo_manifest: Option<PathBuf>,
    pub web_cargo_manifest: Option<PathBuf>,
    #[serde(default)]
    pub targets: Vec<ApplicationTarget>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApplicationTarget {
    Native,
    Web,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct AssetsManifest {
    #[serde(default)]
    pub sources: Vec<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct WorkerBundleManifest {
    pub id: String,
    pub web_artifact: PathBuf,
    pub native_cargo_manifest: PathBuf,
    pub entries: Vec<String>,
    #[serde(default)]
    pub host_services: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PackageSettings {
    #[serde(default)]
    pub macos: MacOsPackageSettings,
    #[serde(default)]
    pub windows: WindowsPackageSettings,
    #[serde(default)]
    pub linux: LinuxPackageSettings,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct MacOsPackageSettings {
    pub minimum_version: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct WindowsPackageSettings {
    pub publisher: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LinuxPackageSettings {
    #[serde(default)]
    pub categories: Vec<String>,
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<FuiManifest> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| Error::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest: FuiManifest =
        toml::from_str(&source).map_err(|source| Error::ParseFuiManifest {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
    if manifest.schema_version != FUI_MANIFEST_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchemaVersion {
            found: manifest.schema_version,
            supported: FUI_MANIFEST_SCHEMA_VERSION,
        });
    }
    validate_workers(&manifest.workers)?;
    Ok(manifest)
}

fn validate_workers(workers: &[WorkerBundleManifest]) -> Result<()> {
    let mut bundle_ids = HashSet::new();
    let mut entry_names = HashSet::new();
    for worker in workers {
        if worker.id.trim().is_empty() {
            return Err(Error::InvalidWorkerManifest(
                "bundle id must not be empty".to_owned(),
            ));
        }
        if !bundle_ids.insert(worker.id.as_str()) {
            return Err(Error::InvalidWorkerManifest(format!(
                "duplicate bundle id {:?}",
                worker.id
            )));
        }
        validate_worker_path(&worker.web_artifact, "web-artifact", &worker.id)?;
        if worker
            .web_artifact
            .extension()
            .and_then(|value| value.to_str())
            != Some("wasm")
        {
            return Err(Error::InvalidWorkerManifest(format!(
                "bundle {:?} web-artifact must end in .wasm",
                worker.id
            )));
        }
        validate_worker_path(
            &worker.native_cargo_manifest,
            "native-cargo-manifest",
            &worker.id,
        )?;
        if worker.entries.is_empty() {
            return Err(Error::InvalidWorkerManifest(format!(
                "bundle {:?} must declare at least one entry",
                worker.id
            )));
        }
        for entry in &worker.entries {
            if entry.trim().is_empty() {
                return Err(Error::InvalidWorkerManifest(format!(
                    "bundle {:?} contains an empty entry name",
                    worker.id
                )));
            }
            if !is_native_symbol(entry) {
                return Err(Error::InvalidWorkerManifest(format!(
                    "worker entry {entry:?} must be an ASCII C identifier"
                )));
            }
            if !entry_names.insert(entry.as_str()) {
                return Err(Error::InvalidWorkerManifest(format!(
                    "duplicate worker entry {entry:?}"
                )));
            }
        }
        let mut services = HashSet::new();
        for service in &worker.host_services {
            if service.trim().is_empty() || !services.insert(service.as_str()) {
                return Err(Error::InvalidWorkerManifest(format!(
                    "bundle {:?} contains an empty or duplicate host service",
                    worker.id
                )));
            }
        }
    }
    Ok(())
}

fn validate_worker_path(path: &Path, field: &str, bundle: &str) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::InvalidWorkerManifest(format!(
            "bundle {bundle:?} {field} must be a project-relative path without parent traversal"
        )));
    }
    Ok(())
}

fn is_native_symbol(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}
