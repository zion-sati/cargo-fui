mod build;
mod cli;
mod error;
mod manifest;
mod native_bundle;
mod package_contract;
mod runtime_acquisition;
mod scaffold;
mod signing;
mod target;

pub use build::{build_project, dev_project, package_project, BuildOptions, BuildResult};
pub use cli::{run_cli, CliIo};
pub use effindom_native_packaging::{
    assemble_universal_macos_app, create_appimage, create_dmg, create_msix, create_release_archive,
    encode_application_icon_png, encode_browser_favicon, encode_linux_desktop_entry,
    encode_linux_hicolor, encode_macos_icns, encode_macos_info_plist, encode_windows_ico,
    encode_windows_resource_script, extract_release_archive, load_icon_source, verify_bundle,
    AppImageInputs, ApplicationMetadata, BundleFile, BundleFileRole, DistributionArtifact,
    DmgInputs, IconAlpha, IconQualityWarning, IconRaster, IconRasterSet, IconSource,
    IconSourceFormat, MsixInputs, NativeLibrarySearch, NativePackageLayout, NativePlatform,
    NativeVersion, OverwritePolicy, PackageArchitecture, PackageBuildMode, PackageMetadata,
    PackageOperatingSystem, PackageRecord, PackageRecordFile, PackagingInputs,
    ReleaseArchiveArtifact, ReleaseArchiveFormat, ReleaseArchiveSpec, UniversalMacOsArtifact,
    UniversalMacOsInputs, CANONICAL_ICON_SIZES, MINIMUM_PNG_ICON_SIZE, RECOMMENDED_PNG_ICON_SIZE,
};
pub use effindom_native_packaging::{
    decode_native_runtime_bundle_manifest, decode_native_runtime_release_manifest,
    encode_native_runtime_bundle_manifest, encode_native_runtime_release_manifest,
    extract_native_runtime_artifact, verify_native_runtime_directory, NativeRuntimeArchiveFormat,
    NativeRuntimeArtifact, NativeRuntimeBundleManifest, NativeRuntimeFile, NativeRuntimeFileRole,
    NativeRuntimeMinimumOs, NativeRuntimeReleaseManifest, NativeRuntimeTarget,
    NATIVE_RUNTIME_MANIFEST_SCHEMA_VERSION, REQUIRED_NATIVE_RUNTIME_TARGETS,
};
pub use effindom_native_packaging::{
    Error as NativePackagingError, IconArtifact, Result as NativePackagingResult,
};
pub use error::{Error, Result};
pub use manifest::{
    load_manifest, ApplicationManifest, ApplicationTarget, AssetsManifest, FuiManifest,
    LinuxPackageSettings, MacOsPackageSettings, PackageSettings, WindowsPackageSettings,
    FUI_MANIFEST_SCHEMA_VERSION,
};
pub use native_bundle::{
    stage_native_bundle, NativeBuildOutput, NativeLibraryOutput, StagedNativeBundle,
};
pub use package_contract::{
    resolve_package_contract, BuildProfile, PackageContract, PackageLayout, PackageRequest,
    ResolvedApplication, RuntimeAbi, SigningMode, CORE_ABI_VERSION, UI_ABI_VERSION,
};
pub use runtime_acquisition::{
    acquire_native_runtime, clean_native_runtime_cache, list_native_runtime_cache,
    runtime_requirement_from_cargo_metadata, AcquiredNativeRuntime, NativeRuntimeAcquisition,
    NativeRuntimeCacheEntry, NativeRuntimeSource, RuntimeDownloader, RuntimeRequirement,
    UreqRuntimeDownloader, DEFAULT_NATIVE_RUNTIME_RELEASE_BASE_URL,
};
pub use scaffold::{create_project, NewProjectOptions, ProjectTemplate};
pub use signing::{
    sign_macos_application, sign_windows_msix, ArtifactSigningPurpose, MacOsNotarization,
    MacOsSigningInputs, SignedArtifact, SignedArtifactRecord, WindowsCertificateKind,
    WindowsSigningInputs,
};
pub use target::{Architecture, OperatingSystem, TargetPlatform};
