use crate::{Error, Result};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OperatingSystem {
    MacOs,
    Windows,
    Linux,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Architecture {
    X64,
    Arm64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TargetPlatform {
    pub triple: String,
    pub operating_system: OperatingSystem,
    pub architecture: Architecture,
}

impl TargetPlatform {
    pub fn parse(triple: impl Into<String>) -> Result<Self> {
        let triple = triple.into();
        let platform = match triple.as_str() {
            "aarch64-apple-darwin" => (OperatingSystem::MacOs, Architecture::Arm64),
            "x86_64-apple-darwin" => (OperatingSystem::MacOs, Architecture::X64),
            "aarch64-pc-windows-msvc" => (OperatingSystem::Windows, Architecture::Arm64),
            "x86_64-pc-windows-msvc" => (OperatingSystem::Windows, Architecture::X64),
            "aarch64-unknown-linux-gnu" => (OperatingSystem::Linux, Architecture::Arm64),
            "x86_64-unknown-linux-gnu" => (OperatingSystem::Linux, Architecture::X64),
            _ => return Err(Error::UnsupportedTarget(triple)),
        };
        Ok(Self {
            triple,
            operating_system: platform.0,
            architecture: platform.1,
        })
    }
}
