use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
    Ok(manifest)
}
