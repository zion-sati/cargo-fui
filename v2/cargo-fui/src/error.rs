use std::fmt;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseFuiManifest {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    ParseCargoManifest {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    UnsupportedSchemaVersion {
        found: u32,
        supported: u32,
    },
    InvalidWorkerManifest(String),
    MissingCargoPackage {
        path: PathBuf,
    },
    MissingCargoField {
        path: PathBuf,
        field: &'static str,
    },
    InvalidCargoPackageVersion {
        path: PathBuf,
        version: String,
        source: semver::Error,
    },
    NativeVersionComponentTooLarge {
        version: String,
        component: &'static str,
    },
    NativePackaging(effindom_native_packaging::Error),
    InvalidApplicationIdentifier(String),
    InvalidIconPath(PathBuf),
    MissingApplicationIcon,
    UnsupportedTarget(String),
    MissingSigningMetadata {
        target: String,
        field: &'static str,
    },
    UnsupportedSigningTarget(String),
    SerializePackageRecord(serde_json::Error),
    SigningConfiguration(String),
    SigningIo {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    SigningTool {
        tool: &'static str,
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    SerializeSigningRecord(serde_json::Error),
    RuntimeIo {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    RuntimeDownload {
        url: String,
        message: String,
    },
    RuntimeUnavailable(String),
    RuntimeRequirement(String),
    Cli(String),
    ProjectNotFound(PathBuf),
    ProjectExists(PathBuf),
    Process {
        program: String,
        message: String,
    },
    SerializeLinkMetadata(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::ParseFuiManifest { path, source } => {
                write!(formatter, "failed to parse FUI manifest {}: {source}", path.display())
            }
            Self::ParseCargoManifest { path, source } => {
                write!(formatter, "failed to parse Cargo manifest {}: {source}", path.display())
            }
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                formatter,
                "fui.toml schema version {found} is not supported; expected {supported}"
            ),
            Self::InvalidWorkerManifest(message) => {
                write!(formatter, "invalid fui.toml worker declaration: {message}")
            }
            Self::MissingCargoPackage { path } => write!(
                formatter,
                "Cargo manifest {} has no [package] table; point application.cargo-manifest at an application package manifest",
                path.display()
            ),
            Self::MissingCargoField { path, field } => write!(
                formatter,
                "Cargo manifest {} is missing string field [package].{field}",
                path.display()
            ),
            Self::InvalidCargoPackageVersion { path, version, source } => write!(
                formatter,
                "Cargo package version {version:?} in {} cannot be used for native metadata: {source}",
                path.display()
            ),
            Self::NativeVersionComponentTooLarge { version, component } => write!(
                formatter,
                "Cargo package version {version:?} has {component} outside the native metadata range 0..=65535"
            ),
            Self::NativePackaging(source) => source.fmt(formatter),
            Self::InvalidApplicationIdentifier(identifier) => write!(
                formatter,
                "application identifier {identifier:?} must contain at least two dot-separated ASCII alphanumeric or hyphen components"
            ),
            Self::InvalidIconPath(path) => write!(
                formatter,
                "application icon {} must be an SVG or PNG source file",
                path.display()
            ),
            Self::MissingApplicationIcon => {
                write!(formatter, "native packaging requires application.icon in fui.toml")
            }
            Self::UnsupportedTarget(target) => write!(
                formatter,
                "target {target:?} is not supported; use a macOS, Windows MSVC, or Linux GNU x64/ARM64 target"
            ),
            Self::MissingSigningMetadata { target, field } => write!(
                formatter,
                "signed {target} packaging requires fui.toml field {field}"
            ),
            Self::UnsupportedSigningTarget(target) => {
                write!(formatter, "signed packaging is not yet supported for {target}")
            }
            Self::SerializePackageRecord(source) => {
                write!(formatter, "failed to serialize package record: {source}")
            }
            Self::SigningConfiguration(message) => {
                write!(formatter, "invalid signing configuration: {message}")
            }
            Self::SigningIo { operation, path, source } => {
                write!(formatter, "failed to {operation} {}: {source}", path.display())
            }
            Self::SigningTool { tool, operation, path, message } => {
                write!(formatter, "{tool} failed to {operation} {}: {message}", path.display())
            }
            Self::SerializeSigningRecord(source) => {
                write!(formatter, "failed to serialize signed-artifact record: {source}")
            }
            Self::RuntimeIo { operation, path, source } => {
                write!(formatter, "failed to {operation} {}: {source}", path.display())
            }
            Self::RuntimeDownload { url, message } => {
                write!(formatter, "failed to download native runtime {url}: {message}")
            }
            Self::RuntimeUnavailable(message) => {
                write!(formatter, "native runtime is unavailable: {message}")
            }
            Self::RuntimeRequirement(message) => {
                write!(formatter, "invalid native runtime requirement: {message}")
            }
            Self::Cli(message) => formatter.write_str(message),
            Self::ProjectNotFound(path) => write!(
                formatter,
                "no fui.toml was found at or above {}; run this command in a FUI project",
                path.display()
            ),
            Self::ProjectExists(path) => write!(
                formatter,
                "target directory {} is not empty",
                path.display()
            ),
            Self::Process { program, message } => {
                write!(formatter, "{program} failed: {message}")
            }
            Self::SerializeLinkMetadata(source) => {
                write!(formatter, "failed to parse native runtime link metadata: {source}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::ParseFuiManifest { source, .. } => Some(source.as_ref()),
            Self::ParseCargoManifest { source, .. } => Some(source.as_ref()),
            Self::InvalidCargoPackageVersion { source, .. } => Some(source),
            Self::NativePackaging(source) => Some(source),
            Self::SerializePackageRecord(source) => Some(source),
            Self::SigningIo { source, .. } => Some(source),
            Self::RuntimeIo { source, .. } => Some(source),
            Self::SerializeSigningRecord(source) => Some(source),
            Self::SerializeLinkMetadata(source) => Some(source),
            _ => None,
        }
    }
}

impl From<effindom_native_packaging::Error> for Error {
    fn from(source: effindom_native_packaging::Error) -> Self {
        Self::NativePackaging(source)
    }
}
