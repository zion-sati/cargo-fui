use crate::{
    acquire_native_runtime, create_appimage, create_dmg, create_msix, encode_browser_favicon,
    generate_native_worker_registry, load_icon_source, load_manifest, resolve_package_contract,
    runtime_requirement_from_cargo_metadata, stage_native_bundle, AppImageInputs,
    ApplicationTarget, BuildProfile, DmgInputs, Error, MsixInputs, NativeBuildOutput,
    NativeLibraryOutput, NativeRuntimeAcquisition, NativeRuntimeTarget, NativeWorkerRegistryEntry,
    OperatingSystem, OverwritePolicy, PackageContract, PackageRequest, Result, SigningMode,
    UreqRuntimeDownloader, WorkerBundleManifest, DEFAULT_NATIVE_RUNTIME_RELEASE_BASE_URL,
};
use serde::Deserialize;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    pub project_root: PathBuf,
    pub profile: BuildProfile,
    pub offline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildResult {
    pub target: ApplicationTarget,
    pub path: PathBuf,
    pub contract: Option<PackageContract>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeLinkMetadata {
    schema_version: u32,
    target: String,
    libraries: Vec<String>,
    system_libraries: Vec<String>,
    runtime_library_directory: String,
    include_directory: String,
    launcher: String,
}

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    target_directory: PathBuf,
}

#[derive(Deserialize)]
struct CargoPackage {
    manifest_path: PathBuf,
    targets: Vec<CargoTarget>,
}

#[derive(Deserialize)]
struct CargoTarget {
    name: String,
    crate_types: Vec<String>,
}

pub fn build_project(options: &BuildOptions) -> Result<Vec<BuildResult>> {
    let manifest = load_manifest(options.project_root.join("fui.toml"))?;
    let targets = if manifest.application.targets.is_empty() {
        vec![ApplicationTarget::Native]
    } else {
        manifest.application.targets
    };
    let mut outputs = Vec::with_capacity(targets.len());
    for target in targets {
        outputs.push(match target {
            ApplicationTarget::Native => build_native(options)?,
            ApplicationTarget::Web => build_web(options)?,
        });
    }
    Ok(outputs)
}

pub fn package_project(options: &BuildOptions) -> Result<PathBuf> {
    let targets = project_targets(options)?;
    if !targets.contains(&ApplicationTarget::Native) {
        return Err(Error::Cli(
            "cargo fui package requires a native or universal project".into(),
        ));
    }
    let native = build_native(options)?;
    let contract = native
        .contract
        .as_ref()
        .expect("native build has package contract");
    let icon = contract
        .application
        .source_icon
        .as_ref()
        .ok_or(Error::MissingApplicationIcon)?;
    let icons = load_icon_source(icon)?.canonical_rasters()?;
    let metadata = contract.native_metadata()?;
    let destination_root = options.project_root.join("dist");
    fs::create_dir_all(&destination_root)
        .map_err(|source| io_error("create package output directory", &destination_root, source))?;
    let package_record = contract
        .layout
        .package_record
        .strip_prefix(&contract.layout.root)
        .expect("package record belongs to layout")
        .to_path_buf();
    let executable = contract
        .layout
        .executable
        .strip_prefix(&contract.layout.root)
        .expect("executable belongs to layout")
        .to_path_buf();
    let base_name = format!(
        "{}-{}-{}",
        contract.application.name, contract.application.version, contract.target.triple
    );
    match contract.target.operating_system {
        OperatingSystem::MacOs => {
            let destination = destination_root.join(format!("{base_name}.dmg"));
            create_dmg(
                &DmgInputs {
                    app_bundle: native.path,
                    package_record,
                    destination: destination.clone(),
                    volume_name: contract.application.caption.clone(),
                    hdiutil: PathBuf::from("hdiutil"),
                },
                OverwritePolicy::Replace,
            )?;
            Ok(destination)
        }
        OperatingSystem::Windows => {
            let destination = destination_root.join(format!("{base_name}.msix"));
            create_msix(
                &MsixInputs {
                    bundle_root: native.path,
                    package_record,
                    destination: destination.clone(),
                    executable,
                    metadata,
                    publisher: contract
                        .platform_settings
                        .windows
                        .publisher
                        .clone()
                        .ok_or_else(|| Error::MissingSigningMetadata {
                            target: "Windows".into(),
                            field: "package.windows.publisher",
                        })?,
                    publisher_display_name: contract.application.caption.clone(),
                    icons,
                    makeappx: PathBuf::from("makeappx"),
                },
                OverwritePolicy::Replace,
            )?;
            Ok(destination)
        }
        OperatingSystem::Linux => {
            let destination = destination_root.join(format!("{base_name}.AppImage"));
            create_appimage(
                &AppImageInputs {
                    bundle_root: native.path,
                    package_record,
                    destination: destination.clone(),
                    executable,
                    metadata,
                    categories: contract.platform_settings.linux.categories.clone(),
                    icons,
                    appimagetool: PathBuf::from("appimagetool"),
                    unsquashfs: PathBuf::from("unsquashfs"),
                },
                OverwritePolicy::Replace,
            )?;
            Ok(destination)
        }
    }
}

pub fn dev_project(options: &BuildOptions, output: impl Fn(&str)) -> Result<()> {
    let targets = project_targets(options)?;
    if targets.contains(&ApplicationTarget::Web) {
        let web = build_web(options)?;
        return serve_web(&web.path, output);
    }
    if !targets.contains(&ApplicationTarget::Native) {
        return Err(Error::Cli("project has no runnable target".into()));
    }
    let native = build_native(options)?;
    let contract = native.contract.as_ref().expect("native build has contract");
    let executable = native.path.join(
        contract
            .layout
            .executable
            .strip_prefix(&contract.layout.root)
            .expect("executable belongs to bundle"),
    );
    run_status(
        &mut Command::new(&executable),
        &executable.display().to_string(),
    )
}

fn project_targets(options: &BuildOptions) -> Result<Vec<ApplicationTarget>> {
    let manifest = load_manifest(options.project_root.join("fui.toml"))?;
    Ok(if manifest.application.targets.is_empty() {
        vec![ApplicationTarget::Native]
    } else {
        manifest.application.targets
    })
}

fn build_native(options: &BuildOptions) -> Result<BuildResult> {
    let target_triple = host_target()?;
    let fui_manifest = load_manifest(options.project_root.join("fui.toml"))?;
    let contract = resolve_package_contract(
        options.project_root.join("fui.toml"),
        PackageRequest::new(&target_triple, options.profile, SigningMode::Unsigned),
    )?;
    let metadata_bytes = cargo_output_with_manifest(
        &options.project_root,
        &contract.application.cargo_manifest,
        ["metadata", "--format-version", "1"],
        "cargo metadata",
    )?;
    let requirement = runtime_requirement_from_cargo_metadata(&metadata_bytes)?;
    let metadata: CargoMetadata =
        serde_json::from_slice(&metadata_bytes).map_err(Error::SerializeLinkMetadata)?;
    let runtime = acquire_native_runtime(
        &NativeRuntimeAcquisition {
            requirement,
            target: native_runtime_target(&target_triple)?,
            cache_root: runtime_cache_root(),
            override_root: env::var_os("EFFINDOM_NATIVE_RUNTIME_DIR").map(PathBuf::from),
            offline: options.offline,
            release_base_url: env::var("EFFINDOM_NATIVE_RUNTIME_RELEASE_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_NATIVE_RUNTIME_RELEASE_BASE_URL.to_string()),
        },
        &UreqRuntimeDownloader,
    )?;
    let package = application_package(&metadata, &contract.application.cargo_manifest)?;
    let crate_name = package
        .targets
        .iter()
        .find(|target| target.crate_types.iter().any(|kind| kind == "staticlib"))
        .map(|target| target.name.replace('-', "_"))
        .ok_or_else(|| Error::Cli("native projects require a staticlib crate target".into()))?;
    let profile = profile_name(options.profile);
    let generated_root = options
        .project_root
        .join("target/fui")
        .join(&target_triple)
        .join(profile)
        .join("generated");
    recreate_directory(&generated_root)?;
    let worker_registry = generated_root.join("fui_native_worker_registry.rs");
    let worker_entries =
        native_worker_registry_entries(&options.project_root, &metadata, &fui_manifest.workers)?;
    fs::write(
        &worker_registry,
        generate_native_worker_registry(&worker_entries).source,
    )
    .map_err(|source| io_error("write native worker registry", &worker_registry, source))?;
    let mut cargo = Command::new("cargo");
    let macos_minimum_version = macos_minimum_version(&contract);
    cargo
        .current_dir(&options.project_root)
        .arg("build")
        .arg("--manifest-path")
        .arg(&contract.application.cargo_manifest)
        .args(["--target", &target_triple]);
    cargo
        .env("FUI_NATIVE_WORKER_REGISTRY_RS", &worker_registry)
        .env(
            "RUSTFLAGS",
            native_worker_rustflags(env::var("RUSTFLAGS").ok().as_deref()),
        );
    if let Some(version) = macos_minimum_version {
        cargo.env("MACOSX_DEPLOYMENT_TARGET", version);
    }
    if paths_identify_same_file(
        &contract.application.cargo_manifest,
        &options.project_root.join("Cargo.toml"),
    ) {
        cargo.args(["--features", "native"]);
    }
    if options.profile == BuildProfile::Release {
        cargo.arg("--release");
    }
    if options.offline {
        cargo.arg("--offline");
    }
    run_status(&mut cargo, "cargo build")?;
    let rust_library = metadata
        .target_directory
        .join(&target_triple)
        .join(profile)
        .join(static_library_name(
            &crate_name,
            contract.target.operating_system,
        ));
    let raw_root = options
        .project_root
        .join("target/fui")
        .join(&target_triple)
        .join(profile)
        .join("raw");
    recreate_directory(&raw_root)?;
    let executable = raw_root.join(executable_name(
        &contract.application.name,
        contract.target.operating_system,
    ));
    link_native(
        &runtime.root,
        &rust_library,
        &executable,
        contract.target.operating_system,
        macos_minimum_version,
    )?;

    let app_resources = raw_root.join("application-resources");
    fs::create_dir_all(&app_resources)
        .map_err(|source| io_error("create application resources", &app_resources, source))?;
    copy_application_assets(&contract, &app_resources)?;
    let runtime_resources = runtime.root.join("runtime/assets");
    let runtime_libraries = collect_libraries(&runtime.root.join("runtime/lib"))?;
    let package_parent = options
        .project_root
        .join("target/fui")
        .join(&target_triple)
        .join(profile)
        .join("bundle");
    recreate_directory(&package_parent)?;
    let staged = stage_native_bundle(
        &contract,
        &NativeBuildOutput {
            application_executable: executable,
            effindom_runtime_libraries: runtime_libraries,
            third_party_libraries: Vec::new(),
            runtime_resources,
            application_resources: app_resources,
        },
        &package_parent,
        OverwritePolicy::Reject,
    )?;
    Ok(BuildResult {
        target: ApplicationTarget::Native,
        path: staged.root,
        contract: Some(contract),
    })
}

fn build_web(options: &BuildOptions) -> Result<BuildResult> {
    let fui_manifest = load_manifest(options.project_root.join("fui.toml"))?;
    let web_manifest = options.project_root.join(
        fui_manifest
            .application
            .web_cargo_manifest
            .as_deref()
            .unwrap_or_else(|| Path::new("Cargo.toml")),
    );
    let package_json = options.project_root.join("package.json");
    if !package_json.is_file() {
        return Err(Error::Cli(
            "web builds require package.json from the web or universal template".into(),
        ));
    }
    if !options.project_root.join("node_modules").is_dir() {
        let mut install = Command::new("npm");
        install.current_dir(&options.project_root).arg("install");
        if options.offline {
            install.arg("--offline");
        }
        run_status(&mut install, "npm install")?;
    }
    let mut assets = Command::new("npm");
    assets
        .current_dir(&options.project_root)
        .args(["run", "build:assets"]);
    run_status(&mut assets, "npm run build:assets")?;
    if let Some(icon) = &fui_manifest.application.icon {
        let icon = options.project_root.join(icon);
        let favicon = encode_browser_favicon(&load_icon_source(&icon)?.canonical_rasters()?)?;
        let destination = options.project_root.join("public/favicon.ico");
        fs::write(&destination, favicon)
            .map_err(|source| io_error("write browser favicon", &destination, source))?;
    }
    build_web_workers(options, &fui_manifest.workers)?;
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(&options.project_root)
        .arg("build")
        .arg("--manifest-path")
        .arg(&web_manifest)
        .args(["--target", "wasm32-unknown-unknown"]);
    if options.profile == BuildProfile::Release {
        cargo.arg("--release");
    }
    if options.offline {
        cargo.arg("--offline");
    }
    run_status(&mut cargo, "cargo build for wasm32")?;
    let metadata_bytes = cargo_output_with_manifest(
        &options.project_root,
        &web_manifest,
        ["metadata", "--format-version", "1", "--no-deps"],
        "cargo metadata",
    )?;
    let metadata: CargoMetadata =
        serde_json::from_slice(&metadata_bytes).map_err(Error::SerializeLinkMetadata)?;
    let package = application_package(&metadata, &web_manifest)?;
    let crate_name = package
        .targets
        .iter()
        .find(|target| target.crate_types.iter().any(|kind| kind == "cdylib"))
        .map(|target| target.name.replace('-', "_"))
        .ok_or_else(|| Error::Cli("web projects require a cdylib crate target".into()))?;
    let wasm = metadata
        .target_directory
        .join("wasm32-unknown-unknown")
        .join(profile_name(options.profile))
        .join(format!("{crate_name}.wasm"));
    fs::copy(&wasm, options.project_root.join("public/app.wasm"))
        .map_err(|source| io_error("stage application WebAssembly", &wasm, source))?;
    let mut harness = Command::new("npm");
    harness
        .current_dir(&options.project_root)
        .args(["run", "build:harness"]);
    run_status(&mut harness, "npm run build:harness")?;
    Ok(BuildResult {
        target: ApplicationTarget::Web,
        path: options.project_root.join("public"),
        contract: None,
    })
}

fn build_web_workers(options: &BuildOptions, workers: &[WorkerBundleManifest]) -> Result<()> {
    for worker in workers {
        let manifest = options.project_root.join(&worker.native_cargo_manifest);
        let mut cargo = Command::new("cargo");
        cargo
            .current_dir(&options.project_root)
            .arg("rustc")
            .arg("--manifest-path")
            .arg(&manifest)
            .args([
                "--target",
                "wasm32-unknown-unknown",
                "--lib",
                "--crate-type",
                "cdylib",
            ]);
        if options.profile == BuildProfile::Release {
            cargo.arg("--release");
        }
        if options.offline {
            cargo.arg("--offline");
        }
        run_status(
            &mut cargo,
            &format!("build Worker bundle {:?} for wasm32", worker.id),
        )?;

        let metadata_bytes = cargo_output_with_manifest(
            &options.project_root,
            &manifest,
            ["metadata", "--format-version", "1", "--no-deps"],
            "cargo metadata for Worker bundle",
        )?;
        let metadata: CargoMetadata =
            serde_json::from_slice(&metadata_bytes).map_err(Error::SerializeLinkMetadata)?;
        let package = application_package(&metadata, &manifest)?;
        let crate_name = package
            .targets
            .iter()
            .find(|target| {
                target
                    .crate_types
                    .iter()
                    .any(|kind| kind == "lib" || kind == "rlib")
            })
            .map(|target| target.name.replace('-', "_"))
            .ok_or_else(|| {
                Error::Cli(format!(
                    "worker bundle {:?} requires a Rust library target in {}",
                    worker.id,
                    manifest.display()
                ))
            })?;
        let wasm = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(profile_name(options.profile))
            .join(format!("{crate_name}.wasm"));
        let destination = web_worker_destination(&options.project_root, &worker.web_artifact);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                io_error("create Worker artifact output directory", parent, source)
            })?;
        }
        fs::copy(&wasm, &destination)
            .map_err(|source| io_error("stage Worker WebAssembly", &wasm, source))?;
    }
    Ok(())
}

fn web_worker_destination(project_root: &Path, artifact: &Path) -> PathBuf {
    let relative = artifact
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value),
            _ => None,
        })
        .collect::<PathBuf>();
    project_root.join("public").join(relative)
}

fn native_worker_registry_entries(
    project_root: &Path,
    metadata: &CargoMetadata,
    workers: &[WorkerBundleManifest],
) -> Result<Vec<NativeWorkerRegistryEntry>> {
    let mut entries = Vec::new();
    for worker in workers {
        let manifest = project_root.join(&worker.native_cargo_manifest);
        let package = application_package(metadata, &manifest).map_err(|_| {
            Error::Cli(format!(
                "worker bundle {:?} native crate {} must be a dependency of the application crate",
                worker.id,
                manifest.display()
            ))
        })?;
        let crate_name = package
            .targets
            .iter()
            .find(|target| {
                target
                    .crate_types
                    .iter()
                    .any(|kind| kind == "lib" || kind == "rlib")
            })
            .map(|target| target.name.replace('-', "_"))
            .ok_or_else(|| {
                Error::Cli(format!(
                    "worker bundle {:?} requires a Rust library target in {}",
                    worker.id,
                    manifest.display()
                ))
            })?;
        for entry in &worker.entries {
            entries.push(NativeWorkerRegistryEntry {
                artifact: worker.web_artifact.to_string_lossy().into_owned(),
                entry: entry.clone(),
                native_crate: crate_name.clone(),
                host_services: worker.host_services.clone(),
            });
        }
    }
    Ok(entries)
}

fn native_worker_rustflags(existing: Option<&str>) -> String {
    let required = "--cfg fui_native_worker_registry --check-cfg=cfg(fui_native_worker_registry)";
    match existing.filter(|flags| !flags.trim().is_empty()) {
        Some(flags) => format!("{flags} {required}"),
        None => required.to_owned(),
    }
}

fn link_native(
    runtime: &Path,
    application_library: &Path,
    executable: &Path,
    operating_system: OperatingSystem,
    macos_minimum_version: Option<&str>,
) -> Result<()> {
    let path = runtime.join("sdk/link.json");
    let link: NativeLinkMetadata = serde_json::from_slice(
        &fs::read(&path).map_err(|source| io_error("read runtime link metadata", &path, source))?,
    )
    .map_err(Error::SerializeLinkMetadata)?;
    if link.schema_version != 1 {
        return Err(Error::Cli(format!(
            "unsupported native link metadata schema {}",
            link.schema_version
        )));
    }
    let expected_target = native_runtime_target(&host_target()?)?;
    if link.target != expected_target.as_str() {
        return Err(Error::Cli(format!(
            "native runtime link target {} does not match host {}",
            link.target,
            expected_target.as_str()
        )));
    }
    let launcher = runtime.join(&link.launcher);
    let include = runtime.join(&link.include_directory);
    let libraries = link
        .libraries
        .iter()
        .map(|library| {
            if library == "<application-static-library>" {
                application_library.to_path_buf()
            } else {
                runtime.join(library)
            }
        })
        .collect::<Vec<_>>();
    let mut command = if operating_system == OperatingSystem::Windows {
        let mut command = Command::new(env::var_os("CXX").unwrap_or_else(|| OsString::from("cl")));
        command
            .arg("/nologo")
            .arg("/std:c++17")
            .arg("/EHsc")
            .arg("/MD")
            .arg(&launcher)
            .arg(format!("/I{}", include.display()));
        for library in &libraries {
            command.arg(library);
        }
        for library in &link.system_libraries {
            command.arg(format!("{library}.lib"));
        }
        for library in supplemental_system_libraries(operating_system, &link.system_libraries) {
            command.arg(format!("{library}.lib"));
        }
        command
            .arg("/link")
            .arg(format!("/OUT:{}", executable.display()));
        command
    } else {
        let mut command = Command::new(env::var_os("CXX").unwrap_or_else(|| OsString::from("c++")));
        command
            .arg("-std=c++17")
            .arg(&launcher)
            .arg("-I")
            .arg(&include)
            .arg("-o")
            .arg(executable);
        if let Some(version) = macos_minimum_version {
            command.arg(format!("-mmacosx-version-min={version}"));
        }
        if operating_system == OperatingSystem::Linux {
            command.arg("-Wl,--start-group");
        }
        for library in &libraries {
            command.arg(library);
        }
        if operating_system == OperatingSystem::Linux {
            command.arg("-Wl,--end-group");
        }
        for library in &link.system_libraries {
            if let Some(framework) = library.strip_suffix(".framework") {
                command.args(["-framework", framework]);
            } else {
                command.arg(format!("-l{library}"));
            }
        }
        let rpath = if operating_system == OperatingSystem::MacOs {
            "@executable_path/../Frameworks"
        } else {
            "$ORIGIN/../lib"
        };
        command.arg(format!("-Wl,-rpath,{rpath}"));
        command
    };
    command.current_dir(runtime);
    run_status(&mut command, "native C++ linker")?;
    let _ = &link.runtime_library_directory;
    Ok(())
}

fn macos_minimum_version(contract: &PackageContract) -> Option<&str> {
    deployment_target_for(
        contract.target.operating_system,
        contract.platform_settings.macos.minimum_version.as_deref(),
    )
}

fn deployment_target_for(
    operating_system: OperatingSystem,
    configured: Option<&str>,
) -> Option<&str> {
    (operating_system == OperatingSystem::MacOs)
        .then_some(configured)
        .flatten()
}

fn supplemental_system_libraries(
    operating_system: OperatingSystem,
    declared: &[String],
) -> Vec<&'static str> {
    const WINDOWS_BASELINE: &[&str] = &["user32", "oleaut32"];
    if operating_system != OperatingSystem::Windows {
        return Vec::new();
    }
    WINDOWS_BASELINE
        .iter()
        .copied()
        .filter(|required| {
            !declared
                .iter()
                .any(|library| library.eq_ignore_ascii_case(required))
        })
        .collect()
}

fn application_package<'a>(
    metadata: &'a CargoMetadata,
    manifest: &Path,
) -> Result<&'a CargoPackage> {
    let manifest = fs::canonicalize(manifest)
        .map_err(|source| io_error("resolve application Cargo manifest", manifest, source))?;
    metadata
        .packages
        .iter()
        .find(|package| paths_identify_same_file(&package.manifest_path, &manifest))
        .ok_or_else(|| {
            Error::Cli(format!(
                "Cargo metadata does not contain {}",
                manifest.display()
            ))
        })
}

fn paths_identify_same_file(left: &Path, right: &Path) -> bool {
    let Ok(left) = fs::canonicalize(left) else {
        return false;
    };
    let Ok(right) = fs::canonicalize(right) else {
        return false;
    };
    left == right
}

fn copy_application_assets(contract: &PackageContract, destination: &Path) -> Result<()> {
    for source in &contract.application.asset_sources {
        if source.is_dir() {
            copy_tree_contents(source, destination)?;
        } else if source.is_file() {
            let name = source.file_name().ok_or_else(|| {
                Error::Cli(format!("asset {} has no file name", source.display()))
            })?;
            fs::copy(source, destination.join(name))
                .map_err(|error| io_error("copy application asset", source, error))?;
        } else {
            return Err(Error::Cli(format!(
                "asset source {} does not exist",
                source.display()
            )));
        }
    }
    Ok(())
}

fn copy_tree_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in
        fs::read_dir(source).map_err(|error| io_error("read asset directory", source, error))?
    {
        let entry = entry.map_err(|error| io_error("read asset entry", source, error))?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| io_error("create asset directory", &target, error))?;
            copy_tree_contents(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)
                .map_err(|error| io_error("copy application asset", &entry.path(), error))?;
        }
    }
    Ok(())
}

fn collect_libraries(root: &Path) -> Result<Vec<NativeLibraryOutput>> {
    let mut output = Vec::new();
    collect_library_tree(root, root, &mut output)?;
    output.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(output)
}

fn collect_library_tree(
    root: &Path,
    directory: &Path,
    output: &mut Vec<NativeLibraryOutput>,
) -> Result<()> {
    for entry in fs::read_dir(directory)
        .map_err(|source| io_error("read runtime library directory", directory, source))?
    {
        let entry =
            entry.map_err(|source| io_error("read runtime library entry", directory, source))?;
        if entry.path().is_dir() {
            collect_library_tree(root, &entry.path(), output)?;
        } else {
            output.push(NativeLibraryOutput::new(
                entry.path(),
                entry.path().strip_prefix(root).expect("library below root"),
            ));
        }
    }
    Ok(())
}

fn host_target() -> Result<String> {
    if let Ok(target) = env::var("CARGO_BUILD_TARGET") {
        return Ok(target);
    }
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|source| process_error("rustc", source.to_string()))?;
    if !output.status.success() {
        return Err(process_error("rustc -vV", text_output(&output)));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_string)
        .ok_or_else(|| process_error("rustc -vV", "output has no host target"))
}

fn native_runtime_target(triple: &str) -> Result<NativeRuntimeTarget> {
    match triple {
        "aarch64-apple-darwin" => Ok(NativeRuntimeTarget::MacosArm64),
        "x86_64-apple-darwin" => Ok(NativeRuntimeTarget::MacosX64),
        "aarch64-pc-windows-msvc" => Ok(NativeRuntimeTarget::WindowsArm64),
        "x86_64-pc-windows-msvc" => Ok(NativeRuntimeTarget::WindowsX64),
        "aarch64-unknown-linux-gnu" => Ok(NativeRuntimeTarget::LinuxArm64),
        "x86_64-unknown-linux-gnu" => Ok(NativeRuntimeTarget::LinuxX64),
        _ => Err(Error::UnsupportedTarget(triple.to_string())),
    }
}

fn runtime_cache_root() -> PathBuf {
    env::var_os("EFFINDOM_NATIVE_RUNTIME_CACHE")
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
        .or_else(|| env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from(".effindom-cache"))
        .join("effindom/native-runtimes")
}

fn cargo_output_with_manifest<const N: usize>(
    root: &Path,
    manifest: &Path,
    arguments: [&str; N],
    label: &str,
) -> Result<Vec<u8>> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(arguments)
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .map_err(|source| process_error(label, source.to_string()))?;
    if !output.status.success() {
        return Err(process_error(label, text_output(&output)));
    }
    Ok(output.stdout)
}

fn run_status(command: &mut Command, label: &str) -> Result<()> {
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|source| process_error(label, source.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(process_error(label, format!("exited with {status}")))
    }
}

fn serve_web(root: &Path, output: impl Fn(&str)) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 8080))
        .map_err(|source| io_error("bind development server", root, source))?;
    output("Serving http://127.0.0.1:8080");
    open_browser("http://127.0.0.1:8080")?;
    for stream in listener.incoming() {
        let mut stream =
            stream.map_err(|source| io_error("accept development request", root, source))?;
        serve_request(root, &mut stream)?;
    }
    Ok(())
}

fn serve_request(root: &Path, stream: &mut TcpStream) -> Result<()> {
    let mut request = [0_u8; 4096];
    let count = stream
        .read(&mut request)
        .map_err(|source| io_error("read development request", root, source))?;
    let line = String::from_utf8_lossy(&request[..count]);
    let request_path = line
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");
    let relative = request_path.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    if relative.split('/').any(|component| component == "..") {
        return write_response(stream, 400, "text/plain", b"Bad request");
    }
    let path = root.join(relative);
    match fs::read(&path) {
        Ok(bytes) => write_response(stream, 200, mime_type(&path), &bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_response(stream, 404, "text/plain", b"Not found")
        }
        Err(source) => Err(io_error("read development asset", &path, source)),
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = if status == 200 {
        "OK"
    } else if status == 404 {
        "Not Found"
    } else {
        "Bad Request"
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|source| {
            io_error(
                "write development response",
                Path::new("127.0.0.1:8080"),
                source,
            )
        })
}

fn open_browser(url: &str) -> Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg(url);
        command
    } else if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    } else {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|source| process_error("open browser", source.to_string()))
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn profile_name(profile: BuildProfile) -> &'static str {
    match profile {
        BuildProfile::Debug => "debug",
        BuildProfile::Release => "release",
    }
}

fn static_library_name(name: &str, operating_system: OperatingSystem) -> String {
    if operating_system == OperatingSystem::Windows {
        format!("{name}.lib")
    } else {
        format!("lib{name}.a")
    }
}

fn executable_name(name: &str, operating_system: OperatingSystem) -> String {
    if operating_system == OperatingSystem::Windows {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn recreate_directory(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|source| io_error("replace build directory", path, source))?;
    }
    fs::create_dir_all(path).map_err(|source| io_error("create build directory", path, source))
}

fn text_output(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        stderr
    }
}

fn process_error(program: impl Into<String>, message: impl Into<String>) -> Error {
    Error::Process {
        program: program.into(),
        message: message.into(),
    }
}

fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> Error {
    Error::RuntimeIo {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        deployment_target_for, paths_identify_same_file, supplemental_system_libraries,
        OperatingSystem,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn manifest_identity_accepts_equivalent_platform_path_representations() {
        let root = std::env::temp_dir().join(format!(
            "cargo-fui-manifest-identity-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let manifest = root.join("Cargo.toml");
        fs::write(
            &manifest,
            "[package]\nname = \"path-test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let equivalent = root.join("nested").join("..").join("Cargo.toml");
        fs::create_dir_all(root.join("nested")).unwrap();

        assert!(paths_identify_same_file(&manifest, &equivalent));
        assert!(!paths_identify_same_file(
            &manifest,
            &PathBuf::from("missing-Cargo.toml")
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn windows_linking_supplies_missing_platform_baseline_libraries() {
        let declared = vec!["shell32".to_string(), "USER32".to_string()];

        assert_eq!(
            supplemental_system_libraries(OperatingSystem::Windows, &declared),
            vec!["oleaut32"]
        );
        assert!(supplemental_system_libraries(OperatingSystem::Linux, &declared).is_empty());
    }

    #[test]
    fn macos_builds_use_the_configured_deployment_target_only_on_macos() {
        assert_eq!(
            deployment_target_for(OperatingSystem::MacOs, Some("13.0")),
            Some("13.0")
        );
        assert_eq!(
            deployment_target_for(OperatingSystem::Linux, Some("13.0")),
            None
        );
        assert_eq!(deployment_target_for(OperatingSystem::MacOs, None), None);
    }
}
