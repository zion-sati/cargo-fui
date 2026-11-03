use crate::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};

const FUI_RS_VERSION: &str = "=0.4.0";
const RUNTIME_VERSION: &str = "0.4.0";
const ICON: &[u8] = include_bytes!("../templates/application-icon.png");
const WORKER_SOURCE: &str = include_str!("../templates/worker.rs");
/// The Binaryen release the web templates pin for `wasm-opt`.
pub(crate) const BINARYEN_VERSION: &str = "132.0.0";
/// A browser downloads every route's WebAssembly before it can start, so projects with a web target build for size;
/// release web builds then optimize and strip each module with Binaryen. Native-only projects keep the speed-optimized
/// default.
const WEB_RELEASE_PROFILE: &str =
    "[profile.release]\nopt-level = \"z\"\nlto = true\ncodegen-units = 1\npanic = \"abort\"\n";
const NATIVE_RELEASE_PROFILE: &str =
    "[profile.release]\nopt-level = 3\nlto = true\ncodegen-units = 1\npanic = \"abort\"\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectTemplate {
    Native,
    Web,
    Universal,
    WebRouted,
    UniversalRouted,
}

impl ProjectTemplate {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "native" => Ok(Self::Native),
            "web" => Ok(Self::Web),
            "universal" => Ok(Self::Universal),
            _ => Err(Error::Cli(format!(
                "unsupported project target {value:?}; use native, web, or universal"
            ))),
        }
    }

    pub fn parse_with_layout(target: &str, layout: &str) -> Result<Self> {
        match (target, layout) {
            ("native", "default") => Ok(Self::Native),
            ("web", "default") => Ok(Self::Web),
            ("universal", "default") => Ok(Self::Universal),
            ("web", "routed") => Ok(Self::WebRouted),
            ("universal", "routed") => Ok(Self::UniversalRouted),
            ("native", "routed") => Err(Error::Cli(
                "the routed template requires a web or universal target".into(),
            )),
            (_, "default" | "routed") => Self::parse(target),
            (_, _) => Err(Error::Cli(format!(
                "unsupported project template {layout:?}; use default or routed"
            ))),
        }
    }

    fn includes_native(self) -> bool {
        matches!(self, Self::Native | Self::Universal | Self::UniversalRouted)
    }

    fn includes_web(self) -> bool {
        matches!(
            self,
            Self::Web | Self::Universal | Self::WebRouted | Self::UniversalRouted
        )
    }

    fn is_routed(self) -> bool {
        matches!(self, Self::WebRouted | Self::UniversalRouted)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewProjectOptions {
    pub destination: PathBuf,
    pub project_name: String,
    pub template: ProjectTemplate,
}

pub fn create_project(options: &NewProjectOptions) -> Result<()> {
    if options.destination.exists()
        && fs::read_dir(&options.destination)
            .map_err(|source| io_error("read target directory", &options.destination, source))?
            .next()
            .is_some()
    {
        return Err(Error::ProjectExists(options.destination.clone()));
    }
    fs::create_dir_all(&options.destination)
        .map_err(|source| io_error("create project directory", &options.destination, source))?;
    let result = write_project(options);
    if result.is_err() {
        let _ = fs::remove_dir_all(&options.destination);
    }
    result
}

fn write_project(options: &NewProjectOptions) -> Result<()> {
    let package = package_name(&options.project_name);
    let crate_name = package.replace('-', "_");
    if options.template.is_routed() {
        return write_routed_project(options, &package, &crate_name);
    }
    if options.template == ProjectTemplate::Universal {
        return write_universal_project(options, &package, &crate_name);
    }
    write_text(
        &options.destination.join("Cargo.toml"),
        &cargo_manifest(&package, &crate_name, options.template),
    )?;
    write_text(
        &options.destination.join("fui.toml"),
        &fui_manifest(&package, options.template),
    )?;
    write_text(
        &options.destination.join("src/lib.rs"),
        &rust_source(&options.project_name, true),
    )?;
    write_text(
        &options.destination.join("src/services/mod.rs"),
        &services_module(options.template),
    )?;
    write_worker_crate(&options.destination, "worker", &package)?;
    if options.template.includes_native() {
        write_text(
            &options.destination.join("src/services/native.rs"),
            include_str!("../templates/services-native.rs"),
        )?;
    }
    if options.template.includes_web() {
        write_text(
            &options.destination.join("src/services/web.rs"),
            include_str!("../templates/services-web.rs"),
        )?;
        for (name, contents) in [
            ("package.json", web_package(&package)),
            (
                "harness.ts",
                include_str!("../templates/harness.ts").to_string(),
            ),
            (
                "index.html",
                include_str!("../templates/index.html")
                    .replace("__CAPTION__", &options.project_name),
            ),
            (
                "scripts/prepare-runtime.mjs",
                include_str!("../templates/prepare-runtime.mjs").to_string(),
            ),
            (
                "loading-overlay-styles.html",
                include_str!("../templates/loading-overlay-styles.html").to_string(),
            ),
            (
                "loading-overlay-body.html",
                include_str!("../templates/loading-overlay-body.html").to_string(),
            ),
        ] {
            write_text(&options.destination.join(name), &contents)?;
        }
    }
    write_common_files(options, options.template)?;
    write_bytes(
        &options.destination.join("assets/application-icon.png"),
        ICON,
    )
}

fn write_routed_project(
    options: &NewProjectOptions,
    package: &str,
    crate_name: &str,
) -> Result<()> {
    let universal = options.template == ProjectTemplate::UniversalRouted;
    let members = if universal {
        "[\"crates/shared\", \"crates/routes/*\", \"crates/web/*\", \"crates/native\", \"crates/worker\"]"
    } else {
        "[\"crates/shared\", \"crates/routes/*\", \"crates/web/*\", \"crates/worker\"]"
    };
    write_text(
        &options.destination.join("Cargo.toml"),
        &format!(
            "[workspace]\nmembers = {members}\nresolver = \"2\"\n\n[workspace.dependencies]\nfui = {{ package = \"fui-rs\", version = \"{FUI_RS_VERSION}\" }}\n\n[profile.dev]\ndebug = 0\npanic = \"abort\"\n\n{WEB_RELEASE_PROFILE}"
        ),
    )?;
    let native_manifest = if universal {
        "cargo-manifest = \"crates/native/Cargo.toml\"\n"
    } else {
        ""
    };
    let targets = if universal {
        "[\"native\", \"web\"]"
    } else {
        "[\"web\"]"
    };
    write_text(
        &options.destination.join("fui.toml"),
        &format!(
            "schema-version = 1\n\n[application]\nidentifier = \"dev.example.{package}\"\ncaption = \"{}\"\nicon = \"assets/application-icon.png\"\n{native_manifest}targets = {targets}\n\n[[web-routes]]\nroute-path = \"/\"\nwasm-artifact = \"home.wasm\"\ncargo-manifest = \"crates/web/home/Cargo.toml\"\n\n[[web-routes]]\nroute-path = \"/settings/\"\nwasm-artifact = \"settings.wasm\"\ncargo-manifest = \"crates/web/settings/Cargo.toml\"\n\n[assets]\nsources = [\"assets\"]\n\n[[workers]]\nid = \"sample\"\nweb-artifact = \"./workers.wasm\"\nnative-cargo-manifest = \"crates/worker/Cargo.toml\"\nentries = [\"sampleWorker\"]\n\n[package.macos]\nminimum-version = \"13.0\"\n\n[package.windows]\npublisher = \"CN=Development\"\n\n[package.linux]\ncategories = [\"Utility\"]\n",
            options.project_name
        ),
    )?;
    write_text(
        &options.destination.join("crates/shared/Cargo.toml"),
        &format!(
            "[package]\nname = \"{package}-shared\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nfui = {{ workspace = true }}\n"
        ),
    )?;
    write_text(
        &options.destination.join("crates/shared/src/lib.rs"),
        include_str!("../templates/routed-shared.rs"),
    )?;
    for (key, title) in [("home", "Home"), ("settings", "Settings")] {
        let route_crate = format!("{crate_name}_{key}");
        write_text(
            &options
                .destination
                .join(format!("crates/routes/{key}/Cargo.toml")),
            &format!(
                "[package]\nname = \"{package}-{key}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{route_crate}\"\ncrate-type = [\"rlib\"]\n\n[dependencies]\nfui = {{ workspace = true }}\n"
            ),
        )?;
        write_text(
            &options
                .destination
                .join(format!("crates/routes/{key}/src/lib.rs")),
            &include_str!("../templates/routed-page.rs")
                .replace("__PAGE_TYPE__", &format!("{title}Page"))
                .replace("__PAGE_TITLE__", title),
        )?;
        write_text(
            &options
                .destination
                .join(format!("crates/web/{key}/Cargo.toml")),
            &format!(
                "[package]\nname = \"{package}-web-{key}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{crate_name}_web_{key}\"\ncrate-type = [\"cdylib\"]\n\n[dependencies]\nfui = {{ workspace = true }}\n{crate_name}_{key} = {{ package = \"{package}-{key}\", path = \"../../routes/{key}\" }}\n{crate_name}_shared = {{ package = \"{package}-shared\", path = \"../../shared\" }}\n"
            ),
        )?;
        write_text(
            &options
                .destination
                .join(format!("crates/web/{key}/src/lib.rs")),
            &include_str!("../templates/routed-web-adapter.rs")
                .replace("__CRATE_NAME__", crate_name)
                .replace("__ROUTE_CRATE__", &route_crate)
                .replace("__PAGE_KEY__", key)
                .replace("__PAGE_TYPE__", &format!("{title}Page"))
                .replace("__PAGE_TITLE__", title)
                .replace("__PROJECT_NAME__", &options.project_name)
                .replace(
                    "__HOME_ACTIVE__",
                    if key == "home" { "true" } else { "false" },
                ),
        )?;
    }
    if universal {
        write_text(
            &options.destination.join("crates/native/Cargo.toml"),
            &format!(
                "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{crate_name}\"\ncrate-type = [\"staticlib\"]\n\n[dependencies]\nfui = {{ workspace = true, features = [\"native-runtime\"] }}\n{crate_name}_home = {{ package = \"{package}-home\", path = \"../routes/home\" }}\n{crate_name}_settings = {{ package = \"{package}-settings\", path = \"../routes/settings\" }}\n{crate_name}_worker = {{ package = \"{package}-worker\", path = \"../worker\" }}\n"
            ),
        )?;
        write_text(
            &options.destination.join("crates/native/src/lib.rs"),
            &include_str!("../templates/routed-native-adapter.rs")
                .replace("__CRATE_NAME__", crate_name)
                .replace("__PROJECT_NAME__", &options.project_name),
        )?;
    }
    write_worker_crate(&options.destination, "crates/worker", package)?;
    write_web_files(options, package)?;
    let routed_index = include_str!("../templates/index.html")
        .replace("__CAPTION__", &options.project_name)
        .replace("\"./", "\"/")
        .replace(
            "data-effindom-canvas-size-source",
            "id=\"fui-routes\" data-effindom-canvas-size-source",
        );
    write_text(&options.destination.join("index.html"), &routed_index)?;
    write_text(
        &options.destination.join("harness.ts"),
        include_str!("../templates/routed-harness.ts"),
    )?;
    write_text(
        &options.destination.join("routes.json"),
        include_str!("../templates/routed-routes.json"),
    )?;
    write_common_files(options, options.template)?;
    write_bytes(
        &options.destination.join("assets/application-icon.png"),
        ICON,
    )
}

fn write_universal_project(
    options: &NewProjectOptions,
    package: &str,
    crate_name: &str,
) -> Result<()> {
    write_text(
        &options.destination.join("Cargo.toml"),
        &format!(
            "[workspace]\nmembers = [\"crates/ui\", \"crates/native\", \"crates/web\", \"crates/worker\"]\nresolver = \"2\"\n\n[profile.dev]\npanic = \"abort\"\n\n{WEB_RELEASE_PROFILE}"
        ),
    )?;
    write_text(
        &options.destination.join("fui.toml"),
        &format!(
            "schema-version = 1\n\n[application]\nidentifier = \"dev.example.{package}\"\ncaption = \"{}\"\nicon = \"assets/application-icon.png\"\ncargo-manifest = \"crates/native/Cargo.toml\"\nweb-cargo-manifest = \"crates/web/Cargo.toml\"\ntargets = [\"native\", \"web\"]\n\n[assets]\nsources = [\"assets\"]\n\n[[workers]]\nid = \"sample\"\nweb-artifact = \"./workers.wasm\"\nnative-cargo-manifest = \"crates/worker/Cargo.toml\"\nentries = [\"sampleWorker\"]\n\n[package.macos]\nminimum-version = \"13.0\"\n\n[package.windows]\npublisher = \"CN=Development\"\n\n[package.linux]\ncategories = [\"Utility\"]\n",
            options.project_name
        ),
    )?;
    write_text(
        &options.destination.join("crates/ui/Cargo.toml"),
        &format!(
            "[package]\nname = \"{package}-ui\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\ndefault = []\nnative = [\"fui/native-runtime\"]\n\n[dependencies]\nfui = {{ package = \"fui-rs\", version = \"{FUI_RS_VERSION}\" }}\n"
        ),
    )?;
    write_text(
        &options.destination.join("crates/ui/src/lib.rs"),
        &rust_source(&options.project_name, false),
    )?;
    write_text(
        &options.destination.join("crates/ui/src/services/mod.rs"),
        &services_module(ProjectTemplate::Universal),
    )?;
    write_text(
        &options.destination.join("crates/ui/src/services/native.rs"),
        include_str!("../templates/services-native.rs"),
    )?;
    write_text(
        &options.destination.join("crates/ui/src/services/web.rs"),
        include_str!("../templates/services-web.rs"),
    )?;
    write_text(
        &options.destination.join("crates/native/Cargo.toml"),
        &adapter_manifest(package, crate_name, "staticlib", true),
    )?;
    write_text(
        &options.destination.join("crates/native/src/lib.rs"),
        &format!("use {crate_name}_ui::App;\n\nfui::fui_app!(App, App::new);\n"),
    )?;
    write_text(
        &options.destination.join("crates/web/Cargo.toml"),
        &adapter_manifest(
            &format!("{package}-web"),
            &format!("{crate_name}_web"),
            "cdylib",
            false,
        ),
    )?;
    write_text(
        &options.destination.join("crates/web/src/lib.rs"),
        &format!("use {crate_name}_ui::App;\n\nfui::fui_app!(App, App::new);\n"),
    )?;
    write_worker_crate(&options.destination, "crates/worker", package)?;
    write_web_files(options, package)?;
    write_common_files(options, ProjectTemplate::Universal)?;
    write_bytes(
        &options.destination.join("assets/application-icon.png"),
        ICON,
    )
}

fn adapter_manifest(package: &str, crate_name: &str, crate_type: &str, native: bool) -> String {
    let base_package = package.trim_end_matches("-web");
    let base_crate = base_package.replace('-', "_");
    let native_feature = if native {
        ", features = [\"native\"]"
    } else {
        ""
    };
    let worker_dependency = if native {
        format!("{base_crate}_worker = {{ package = \"{base_package}-worker\", path = \"../worker\" }}\n")
    } else {
        String::new()
    };
    format!(
        "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{crate_name}\"\ncrate-type = [\"{crate_type}\"]\n\n[dependencies]\nfui = {{ package = \"fui-rs\", version = \"{FUI_RS_VERSION}\" }}\n{base_crate}_ui = {{ package = \"{base_package}-ui\", path = \"../ui\"{native_feature} }}\n{worker_dependency}"
    )
}

fn write_web_files(options: &NewProjectOptions, package: &str) -> Result<()> {
    for (name, contents) in [
        ("package.json", web_package(package)),
        (
            "harness.ts",
            include_str!("../templates/harness.ts").to_string(),
        ),
        (
            "index.html",
            include_str!("../templates/index.html").replace("__CAPTION__", &options.project_name),
        ),
        (
            "scripts/prepare-runtime.mjs",
            include_str!("../templates/prepare-runtime.mjs").to_string(),
        ),
        (
            "loading-overlay-styles.html",
            include_str!("../templates/loading-overlay-styles.html").to_string(),
        ),
        (
            "loading-overlay-body.html",
            include_str!("../templates/loading-overlay-body.html").to_string(),
        ),
    ] {
        write_text(&options.destination.join(name), &contents)?;
    }
    Ok(())
}

fn write_common_files(options: &NewProjectOptions, template: ProjectTemplate) -> Result<()> {
    write_text(
        &options.destination.join("README.md"),
        &readme(&options.project_name, template),
    )?;
    write_text(
        &options.destination.join(".gitignore"),
        "/target\n/node_modules\n/public\n/dist\n.DS_Store\n",
    )?;
    write_text(
        &options.destination.join("fui-config.json"),
        "{\n  \"$schema\": \"https://effindom.dev/schemas/fui-config.schema.json\",\n  \"version\": 1,\n  \"application\": { \"pageZoom\": \"enabled\" },\n  \"web\": {\n    \"loading\": { \"delayMs\": 300, \"minimumVisibleMs\": 300 }\n  }\n}\n",
    )
}

fn services_module(template: ProjectTemplate) -> String {
    let mut source = String::new();
    if template.includes_native() {
        source.push_str("#[cfg(feature = \"native\")]\npub mod native;\n");
    }
    if template.includes_web() {
        source.push_str("#[cfg(target_family = \"wasm\")]\npub mod web;\n");
    }
    source
}

fn cargo_manifest(package: &str, crate_name: &str, template: ProjectTemplate) -> String {
    let crate_types = if template == ProjectTemplate::Native {
        "[\"staticlib\"]"
    } else if template == ProjectTemplate::Web {
        "[\"cdylib\"]"
    } else {
        "[\"cdylib\", \"staticlib\"]"
    };
    let release_profile = if template == ProjectTemplate::Native {
        NATIVE_RELEASE_PROFILE
    } else {
        WEB_RELEASE_PROFILE
    };
    format!(
        "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"{crate_name}\"\ncrate-type = {crate_types}\n\n[features]\ndefault = []\nnative = [\"fui/native-runtime\"]\n\n[dependencies]\nfui = {{ package = \"fui-rs\", version = \"{FUI_RS_VERSION}\" }}\n\n[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\n{crate_name}_worker = {{ package = \"{package}-worker\", path = \"worker\" }}\n\n[profile.dev]\npanic = \"abort\"\n\n{release_profile}\n[workspace]\nmembers = [\"worker\"]\n"
    )
}

fn fui_manifest(package: &str, template: ProjectTemplate) -> String {
    let targets = match template {
        ProjectTemplate::Native => "[\"native\"]",
        ProjectTemplate::Web | ProjectTemplate::WebRouted => "[\"web\"]",
        ProjectTemplate::Universal | ProjectTemplate::UniversalRouted => "[\"native\", \"web\"]",
    };
    format!(
        "schema-version = 1\n\n[application]\nidentifier = \"dev.example.{package}\"\ncaption = \"{package}\"\nicon = \"assets/application-icon.png\"\ntargets = {targets}\n\n[assets]\nsources = [\"assets\"]\n\n[[workers]]\nid = \"sample\"\nweb-artifact = \"./workers.wasm\"\nnative-cargo-manifest = \"worker/Cargo.toml\"\nentries = [\"sampleWorker\"]\n\n[package.macos]\nminimum-version = \"13.0\"\n\n[package.windows]\npublisher = \"CN=Development\"\n\n[package.linux]\ncategories = [\"Utility\"]\n"
    )
}

fn write_worker_crate(root: &Path, relative: &str, package: &str) -> Result<()> {
    let worker_root = root.join(relative);
    write_text(
        &worker_root.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"{package}-worker\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nfui = {{ package = \"fui-rs\", version = \"{FUI_RS_VERSION}\", default-features = false, features = [\"worker-runtime\"] }}\n\n[lib]\nname = \"{}_worker\"\ncrate-type = [\"rlib\"]\n",
            package.replace('-', "_")
        ),
    )?;
    write_text(&worker_root.join("src/lib.rs"), WORKER_SOURCE)
}

fn rust_source(caption: &str, entrypoint: bool) -> String {
    let source = include_str!("../templates/app.rs")
        .replace("__CAPTION__", caption)
        .replace(
            "__APP_ENTRY__",
            if entrypoint {
                "fui_app!(App, App::new);"
            } else {
                ""
            },
        );
    format!("{}\n", source.trim_end())
}

fn web_package(package: &str) -> String {
    format!(
        "{{\n  \"name\": \"{package}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": {{\n    \"build:assets\": \"node scripts/prepare-runtime.mjs\",\n    \"build:harness\": \"esbuild harness.ts --bundle --format=esm --platform=browser --outfile=public/harness.js\"\n  }},\n  \"dependencies\": {{\n    \"@effindomv2/runtime\": \"{RUNTIME_VERSION}\"\n  }},\n  \"devDependencies\": {{\n    \"binaryen\": \"{BINARYEN_VERSION}\",\n    \"esbuild\": \"0.28.1\"\n  }},\n  \"allowScripts\": {{\n    \"esbuild@0.28.1\": true\n  }}\n}}\n"
    )
}

fn readme(name: &str, template: ProjectTemplate) -> String {
    let (summary, requirements, commands, structure) = match template {
        ProjectTemplate::Native => (
            "A native FUI-RS desktop application. It renders through EffinDOM without Electron or a WebView.",
            "- The stable Rust toolchain from [rustup](https://rustup.rs/)\n- `cargo-fui`, installed with `cargo install --locked cargo-fui`\n- The platform prerequisites documented by [`cargo-fui`](https://github.com/zion-sati/cargo-fui#platform-prerequisites)",
            "```bash\n# Build and run the native app in the default debug profile.\ncargo fui dev\n\n# Produce an optimized native build.\ncargo fui build --release\n\n# Produce the platform package or release archive.\ncargo fui package\n```",
            "- `src/lib.rs` owns the retained UI and application lifecycle.\n- `worker` contains the shared Worker implementation used by native and web builds.\n- `src/services/native.rs` is the boundary for native platform services.\n- `fui.toml` defines application metadata, workers, assets, targets, and packaging settings.\n- `assets/application-icon.png` is the canonical application icon.",
        ),
        ProjectTemplate::Web | ProjectTemplate::WebRouted => (
            "A browser FUI-RS application compiled to WebAssembly.",
            "- The stable Rust toolchain from [rustup](https://rustup.rs/)\n- `cargo-fui`, installed with `cargo install --locked cargo-fui`\n- A current Node.js LTS release and npm",
            "```bash\n# Build the app and serve it locally with rebuild-on-refresh development behavior.\ncargo fui dev\n\n# Produce an optimized browser build in public/.\ncargo fui build --release\n```\n\n`cargo fui package` is intentionally unavailable for web-only projects. Deploy the generated `public/` directory with your normal static-site tooling.",
            "- `src/lib.rs` owns the retained UI and application lifecycle.\n- `worker` contains the Worker implementation compiled to WebAssembly.\n- `src/services/web.rs` is the boundary for browser services.\n- `harness.ts` starts the EffinDOM browser harness.\n- `fui.toml` defines application metadata, workers, assets, and targets.\n- `assets/application-icon.png` is the canonical application icon.",
        ),
        ProjectTemplate::Universal | ProjectTemplate::UniversalRouted => (
            "A universal FUI-RS application with shared retained UI and explicit native and browser adapters. Native rendering does not use Electron or a WebView.",
            "- The stable Rust toolchain from [rustup](https://rustup.rs/)\n- `cargo-fui`, installed with `cargo install --locked cargo-fui`\n- The platform prerequisites documented by [`cargo-fui`](https://github.com/zion-sati/cargo-fui#platform-prerequisites)\n- A current Node.js LTS release and npm for the WebAssembly target",
            "```bash\n# Build and serve the browser adapter during development.\ncargo fui dev\n\n# Produce optimized native and browser builds.\ncargo fui build --release\n\n# Produce the native platform package or release archive.\ncargo fui package\n```",
            "- `crates/ui` owns the shared retained UI and service contracts.\n- `crates/worker` contains the Worker implementation linked natively or compiled to WebAssembly.\n- `crates/native` is the thin native application adapter.\n- `crates/web` is the thin WebAssembly application adapter.\n- `crates/ui/src/services/native.rs` and `web.rs` keep platform services explicit.\n- `fui.toml` defines shared application metadata, workers, assets, targets, and packaging settings.\n- `assets/application-icon.png` is the canonical application icon.",
        ),
    };
    format!(
        "# {name}\n\n{summary}\n\n## Requirements\n\n{requirements}\n\nVerify the tools before continuing:\n\n```bash\nrustc --version\ncargo --version\ncargo fui --help\n```\n\n## Run, build, and package\n\n{commands}\n\n## Project structure\n\n{structure}\n\n## Learn more\n\n- [FUI-RS](https://github.com/zion-sati/fui-rs)\n- [`cargo-fui`](https://github.com/zion-sati/cargo-fui)\n- [Live FUI-RS demo](https://fui-rs-demo.effindom.dev/)\n\nReport framework or tooling problems in the relevant FUI-RS or `cargo-fui` repository.\n"
    )
}

fn package_name(value: &str) -> String {
    let mut output = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if output.is_empty() {
        output = "fui-app".to_string();
    }
    output
}

fn write_text(path: &Path, contents: &str) -> Result<()> {
    write_bytes(path, contents.as_bytes())
}

fn write_bytes(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| io_error("create project subdirectory", parent, source))?;
    }
    fs::write(path, contents).map_err(|source| io_error("write project file", path, source))
}

fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> Error {
    Error::RuntimeIo {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
