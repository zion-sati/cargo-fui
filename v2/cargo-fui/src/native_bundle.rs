use crate::{
    encode_macos_icns, encode_macos_info_plist, load_icon_source, BundleFile, Error,
    OperatingSystem, OverwritePolicy, PackageContract, PackageRecord, PackagingInputs, Result,
};
use effindom_native_packaging::stage_bundle;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_METADATA_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBuildOutput {
    pub application_executable: PathBuf,
    pub effindom_runtime_libraries: Vec<NativeLibraryOutput>,
    pub third_party_libraries: Vec<NativeLibraryOutput>,
    pub runtime_resources: PathBuf,
    pub application_resources: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLibraryOutput {
    pub source: PathBuf,
    pub relative_path: PathBuf,
}

impl NativeLibraryOutput {
    pub fn new(source: impl Into<PathBuf>, relative_path: impl Into<PathBuf>) -> Self {
        Self {
            source: source.into(),
            relative_path: relative_path.into(),
        }
    }

    pub fn from_file(source: impl Into<PathBuf>) -> Result<Self> {
        let source = source.into();
        let name = source.file_name().ok_or_else(|| {
            Error::SigningConfiguration(format!(
                "native library {} has no file name",
                source.display()
            ))
        })?;
        Ok(Self::new(&source, name))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedNativeBundle {
    pub root: PathBuf,
    pub package_record: PathBuf,
    pub record: PackageRecord,
}

pub fn stage_native_bundle(
    contract: &PackageContract,
    output: &NativeBuildOutput,
    destination_parent: impl AsRef<Path>,
    overwrite: OverwritePolicy,
) -> Result<StagedNativeBundle> {
    let destination_parent = destination_parent.as_ref();
    fs::create_dir_all(destination_parent).map_err(|source| Error::SigningIo {
        operation: "create native package destination",
        path: destination_parent.to_path_buf(),
        source,
    })?;
    let root = destination_parent.join(&contract.layout.root);
    let layout_root = &contract.layout.root;
    let executable = relative_to_layout(layout_root, &contract.layout.executable)?;
    let runtime_libraries = relative_to_layout(layout_root, &contract.layout.runtime_libraries)?;
    let runtime_resources = relative_to_layout(layout_root, &contract.layout.runtime_resources)?;
    let application_resources =
        relative_to_layout(layout_root, &contract.layout.application_resources)?;
    let package_record = relative_to_layout(layout_root, &contract.layout.package_record)?;

    let generated = GeneratedMetadata::new(destination_parent)?;
    let inputs = PackagingInputs {
        destination: root.clone(),
        package_record: package_record.clone(),
        metadata: contract.package_metadata(),
        application_executable: BundleFile::new(&output.application_executable, executable),
        effindom_runtime_libraries: map_libraries(
            &libraries_with_platform_aliases(contract, &output.effindom_runtime_libraries),
            &runtime_libraries,
        )?,
        third_party_libraries: map_libraries(
            &libraries_with_platform_aliases(contract, &output.third_party_libraries),
            &runtime_libraries,
        )?,
        runtime_resources: map_tree(&output.runtime_resources, &runtime_resources)?,
        application_resources: map_tree(&output.application_resources, &application_resources)?,
        metadata_artifacts: generated.write(contract)?,
    };
    let record = stage_bundle(&inputs, overwrite)?;
    Ok(StagedNativeBundle {
        root,
        package_record,
        record,
    })
}

fn libraries_with_platform_aliases(
    contract: &PackageContract,
    libraries: &[NativeLibraryOutput],
) -> Vec<NativeLibraryOutput> {
    let mut output = libraries.to_vec();
    if contract.target.operating_system != OperatingSystem::Linux {
        return output;
    }
    for library in libraries {
        if library
            .relative_path
            .file_name()
            .and_then(|name| name.to_str())
            == Some("libSDL3.so")
            && !libraries.iter().any(|candidate| {
                candidate
                    .relative_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    == Some("libSDL3.so.0")
            })
        {
            output.push(NativeLibraryOutput::new(
                &library.source,
                library.relative_path.with_file_name("libSDL3.so.0"),
            ));
        }
    }
    output
}

fn relative_to_layout(root: &Path, path: &Path) -> Result<PathBuf> {
    path.strip_prefix(root).map(Path::to_path_buf).map_err(|_| {
        Error::SigningConfiguration(format!(
            "native package layout path {} is outside {}",
            path.display(),
            root.display()
        ))
    })
}

fn map_libraries(sources: &[NativeLibraryOutput], destination: &Path) -> Result<Vec<BundleFile>> {
    sources
        .iter()
        .map(|library| {
            if library.relative_path.is_absolute()
                || library
                    .relative_path
                    .components()
                    .any(|component| matches!(component, std::path::Component::ParentDir))
            {
                return Err(Error::SigningConfiguration(format!(
                    "native library destination {} must be package-relative",
                    library.relative_path.display()
                )));
            }
            Ok(BundleFile::new(
                &library.source,
                destination.join(&library.relative_path),
            ))
        })
        .collect()
}

fn map_tree(source_root: &Path, destination_root: &Path) -> Result<Vec<BundleFile>> {
    let mut files = Vec::new();
    collect_files(source_root, source_root, destination_root, &mut files)?;
    files.sort_by(|left, right| left.destination.cmp(&right.destination));
    Ok(files)
}

fn collect_files(
    source_root: &Path,
    directory: &Path,
    destination_root: &Path,
    files: &mut Vec<BundleFile>,
) -> Result<()> {
    let entries = fs::read_dir(directory).map_err(|source| Error::SigningIo {
        operation: "read native build resources",
        path: directory.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::SigningIo {
            operation: "read native build resource entry",
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| Error::SigningIo {
            operation: "inspect native build resource",
            path: path.clone(),
            source,
        })?;
        if metadata.is_dir() {
            collect_files(source_root, &path, destination_root, files)?;
        } else if metadata.is_file() && !metadata.file_type().is_symlink() {
            let relative = path
                .strip_prefix(source_root)
                .expect("tree entry below root");
            files.push(BundleFile::new(&path, destination_root.join(relative)));
        } else {
            return Err(Error::SigningConfiguration(format!(
                "native resource {} must be a regular file or directory",
                path.display()
            )));
        }
    }
    Ok(())
}

struct GeneratedMetadata {
    root: PathBuf,
}

impl GeneratedMetadata {
    fn new(parent: &Path) -> Result<Self> {
        let id = NEXT_METADATA_ID.fetch_add(1, Ordering::Relaxed);
        let root = parent.join(format!(
            ".effindom-native-metadata-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root).map_err(|source| Error::SigningIo {
            operation: "create generated native metadata directory",
            path: root.clone(),
            source,
        })?;
        Ok(Self { root })
    }

    fn write(&self, contract: &PackageContract) -> Result<Vec<BundleFile>> {
        if contract.target.operating_system != OperatingSystem::MacOs {
            return Ok(Vec::new());
        }
        let icon = contract
            .application
            .source_icon
            .as_ref()
            .ok_or(Error::MissingApplicationIcon)?;
        let rasters = load_icon_source(icon)?.canonical_rasters()?;
        let metadata = contract.native_metadata()?;
        let minimum_version = contract.platform_settings.macos.minimum_version.as_deref();
        let plist = encode_macos_info_plist(&metadata, minimum_version)?;
        let icns = encode_macos_icns(&rasters)?;
        let plist_path = self.root.join("Info.plist");
        let icon_path = self.root.join("application.icns");
        fs::write(&plist_path, plist.bytes).map_err(|source| Error::SigningIo {
            operation: "write generated macOS Info.plist",
            path: plist_path.clone(),
            source,
        })?;
        fs::write(&icon_path, icns).map_err(|source| Error::SigningIo {
            operation: "write generated macOS icon",
            path: icon_path.clone(),
            source,
        })?;
        Ok(vec![
            BundleFile::new(plist_path, "Contents/Info.plist"),
            BundleFile::new(icon_path, "Contents/Resources/application.icns"),
        ])
    }
}

impl Drop for GeneratedMetadata {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
