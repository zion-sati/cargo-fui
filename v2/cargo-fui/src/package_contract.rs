use crate::{
    load_manifest, ApplicationMetadata, Architecture, Error, FuiManifest, NativePackageLayout,
    NativePlatform, NativeVersion, OperatingSystem, PackageArchitecture, PackageBuildMode,
    PackageMetadata, PackageOperatingSystem, PackageSettings, Result, TargetPlatform,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CORE_ABI_VERSION: u32 = 2;
pub const UI_ABI_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BuildProfile {
    Debug,
    Release,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SigningMode {
    Unsigned,
    Signed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageRequest {
    pub target_triple: String,
    pub profile: BuildProfile,
    pub signing: SigningMode,
}

impl PackageRequest {
    pub fn new(
        target_triple: impl Into<String>,
        profile: BuildProfile,
        signing: SigningMode,
    ) -> Self {
        Self {
            target_triple: target_triple.into(),
            profile,
            signing,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAbi {
    pub core: u32,
    pub ui: u32,
}

pub type PackageLayout = NativePackageLayout;

fn native_platform(operating_system: OperatingSystem) -> NativePlatform {
    match operating_system {
        OperatingSystem::MacOs => NativePlatform::MacOs,
        OperatingSystem::Windows => NativePlatform::Windows,
        OperatingSystem::Linux => NativePlatform::Linux,
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedApplication {
    pub name: String,
    pub version: String,
    pub caption: String,
    pub identifier: String,
    pub cargo_manifest: PathBuf,
    pub source_icon: Option<PathBuf>,
    pub asset_sources: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageContract {
    pub schema_version: u32,
    pub application: ResolvedApplication,
    pub target: TargetPlatform,
    pub profile: BuildProfile,
    pub signing: SigningMode,
    pub runtime_abi: RuntimeAbi,
    pub layout: PackageLayout,
    pub platform_settings: PackageSettingsRecord,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageSettingsRecord {
    pub macos: crate::MacOsPackageSettings,
    pub windows: crate::WindowsPackageSettings,
    pub linux: crate::LinuxPackageSettings,
}

impl From<PackageSettings> for PackageSettingsRecord {
    fn from(settings: PackageSettings) -> Self {
        Self {
            macos: settings.macos,
            windows: settings.windows,
            linux: settings.linux,
        }
    }
}

impl PackageContract {
    pub fn to_pretty_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(Error::SerializePackageRecord)
    }

    pub fn native_metadata(&self) -> Result<ApplicationMetadata> {
        let version = semver::Version::parse(&self.application.version).map_err(|source| {
            Error::InvalidCargoPackageVersion {
                path: self.application.cargo_manifest.clone(),
                version: self.application.version.clone(),
                source,
            }
        })?;
        let component = |value: u64, name: &'static str| {
            u16::try_from(value).map_err(|_| Error::NativeVersionComponentTooLarge {
                version: self.application.version.clone(),
                component: name,
            })
        };
        Ok(ApplicationMetadata::new(
            self.application.name.clone(),
            self.application.version.clone(),
            NativeVersion::new(
                component(version.major, "major")?,
                component(version.minor, "minor")?,
                component(version.patch, "patch")?,
                0,
            ),
            self.application.caption.clone(),
            self.application.identifier.clone(),
        )?)
    }

    pub fn package_metadata(&self) -> PackageMetadata {
        PackageMetadata {
            application_identifier: self.application.identifier.clone(),
            application_version: self.application.version.clone(),
            operating_system: match self.target.operating_system {
                OperatingSystem::MacOs => PackageOperatingSystem::MacOs,
                OperatingSystem::Windows => PackageOperatingSystem::Windows,
                OperatingSystem::Linux => PackageOperatingSystem::Linux,
            },
            architecture: match self.target.architecture {
                Architecture::Arm64 => PackageArchitecture::Arm64,
                Architecture::X64 => PackageArchitecture::X64,
            },
            target_triple: self.target.triple.clone(),
            build_mode: match self.profile {
                BuildProfile::Debug => PackageBuildMode::Debug,
                BuildProfile::Release => PackageBuildMode::Release,
            },
            core_abi: self.runtime_abi.core,
            ui_abi: self.runtime_abi.ui,
        }
    }
}

pub fn resolve_package_contract(
    fui_manifest_path: impl AsRef<Path>,
    request: PackageRequest,
) -> Result<PackageContract> {
    let fui_manifest_path = fui_manifest_path.as_ref();
    let manifest = load_manifest(fui_manifest_path)?;
    let manifest_root = fui_manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let target = TargetPlatform::parse(request.target_triple)?;
    validate_identifier(&manifest.application.identifier)?;
    validate_signing(&manifest, &target, request.signing)?;

    let cargo_manifest = manifest_root.join(
        manifest
            .application
            .cargo_manifest
            .as_deref()
            .unwrap_or_else(|| Path::new("Cargo.toml")),
    );
    let (name, version) = read_cargo_package(&cargo_manifest)?;
    let source_icon = manifest
        .application
        .icon
        .as_ref()
        .map(|path| manifest_root.join(path));
    if let Some(icon) = &source_icon {
        validate_icon_extension(icon)?;
    }
    let application = ResolvedApplication {
        caption: manifest
            .application
            .caption
            .clone()
            .unwrap_or_else(|| name.clone()),
        identifier: manifest.application.identifier.clone(),
        cargo_manifest,
        source_icon,
        asset_sources: manifest
            .assets
            .sources
            .iter()
            .map(|path| manifest_root.join(path))
            .collect(),
        version,
        name: name.clone(),
    };
    Ok(PackageContract {
        schema_version: manifest.schema_version,
        layout: PackageLayout::for_application(&name, native_platform(target.operating_system)),
        application,
        target,
        profile: request.profile,
        signing: request.signing,
        runtime_abi: RuntimeAbi {
            core: CORE_ABI_VERSION,
            ui: UI_ABI_VERSION,
        },
        platform_settings: manifest.package.into(),
    })
}

fn read_cargo_package(path: &Path) -> Result<(String, String)> {
    let source = fs::read_to_string(path).map_err(|source| Error::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let cargo: CargoManifest =
        toml::from_str(&source).map_err(|source| Error::ParseCargoManifest {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
    let package = cargo
        .package
        .ok_or_else(|| Error::MissingCargoPackage { path: path.into() })?;
    let name = package.name.ok_or_else(|| Error::MissingCargoField {
        path: path.into(),
        field: "name",
    })?;
    let version = package.version.ok_or_else(|| Error::MissingCargoField {
        path: path.into(),
        field: "version",
    })?;
    Ok((name, version))
}

#[derive(Deserialize)]
struct CargoManifest {
    package: Option<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: Option<String>,
    version: Option<String>,
}

fn validate_identifier(identifier: &str) -> Result<()> {
    let components: Vec<&str> = identifier.split('.').collect();
    let valid = components.len() >= 2
        && components.iter().all(|component| {
            !component.is_empty()
                && component
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        });
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidApplicationIdentifier(identifier.to_owned()))
    }
}

fn validate_icon_extension(path: &Path) -> Result<()> {
    let valid = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("svg") || extension.eq_ignore_ascii_case("png")
        });
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidIconPath(path.to_path_buf()))
    }
}

fn validate_signing(
    manifest: &FuiManifest,
    target: &TargetPlatform,
    signing: SigningMode,
) -> Result<()> {
    if signing == SigningMode::Unsigned {
        return Ok(());
    }
    match target.operating_system {
        OperatingSystem::MacOs => Ok(()),
        OperatingSystem::Windows if manifest.package.windows.publisher.is_some() => Ok(()),
        OperatingSystem::Windows => Err(Error::MissingSigningMetadata {
            target: "Windows".to_owned(),
            field: "package.windows.publisher",
        }),
        OperatingSystem::Linux => Err(Error::UnsupportedSigningTarget("Linux".to_owned())),
    }
}
