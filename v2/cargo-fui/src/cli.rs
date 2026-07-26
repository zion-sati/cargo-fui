use crate::{
    build_project, create_project, dev_project, package_project, BuildOptions, BuildProfile, Error,
    NewProjectOptions, ProjectTemplate, Result,
};
use std::path::{Path, PathBuf};

pub struct CliIo<'a> {
    output: Box<dyn Fn(&str) + 'a>,
}

impl<'a> CliIo<'a> {
    pub fn new(output: impl Fn(&str) + 'a) -> Self {
        Self {
            output: Box::new(output),
        }
    }

    pub fn stdio() -> Self {
        Self::new(|message| println!("{message}"))
    }

    fn print(&self, message: &str) {
        (self.output)(message);
    }
}

pub fn run_cli(
    arguments: impl IntoIterator<Item = String>,
    cwd: std::io::Result<PathBuf>,
    io: &CliIo<'_>,
) -> Result<()> {
    let cwd = cwd.map_err(|source| Error::RuntimeIo {
        operation: "resolve current directory",
        path: PathBuf::from("."),
        source,
    })?;
    let mut arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments.first().is_some_and(|argument| argument == "fui") {
        arguments.remove(0);
    }
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "new" => command_or_help(&arguments[1..], NEW_USAGE, io, || {
            new_command(&cwd, &arguments[1..], io)
        }),
        "build" => command_or_help(&arguments[1..], BUILD_USAGE, io, || {
            build_command(&cwd, &arguments[1..], io)
        }),
        "dev" => command_or_help(&arguments[1..], DEV_USAGE, io, || {
            dev_command(&cwd, &arguments[1..], io)
        }),
        "package" => command_or_help(&arguments[1..], PACKAGE_USAGE, io, || {
            package_command(&cwd, &arguments[1..], io)
        }),
        "help" | "--help" | "-h" => print_help(arguments.get(1).map(String::as_str), io),
        "--version" | "-V" => {
            io.print(concat!("cargo-fui ", env!("CARGO_PKG_VERSION")));
            Ok(())
        }
        _ => Err(Error::Cli(format!(
            "unknown command {command:?}\n\n{USAGE}"
        ))),
    }
}

const USAGE: &str = "FUI-RS project, development, build, and packaging tooling.\n\nUsage:\n  cargo fui new <path> [--target native|web|universal]\n  cargo fui dev [--release] [--offline]\n  cargo fui build [--release] [--offline]\n  cargo fui package [--debug] [--offline]\n\nRun `cargo fui help <command>` for command-specific help.";

const NEW_USAGE: &str = "Create a FUI-RS application.\n\nUsage:\n  cargo fui new <path> [--target native|web|universal]\n\nTargets:\n  native      Native macOS, Windows, or Linux application; no Node.js required.\n  web         Browser/WebAssembly application; requires Node.js.\n  universal   Shared retained UI with explicit native and web adapters.\n\nThe default target is native.";

const DEV_USAGE: &str = "Build and run or serve the current FUI-RS project.\n\nUsage:\n  cargo fui dev [--release] [--offline]\n\nOptions:\n  --release   Use optimized release output instead of the default debug build.\n  --offline   Require cached dependencies and EffinDOM runtime inputs.";

const BUILD_USAGE: &str = "Build the current FUI-RS project.\n\nUsage:\n  cargo fui build [--release] [--offline]\n\nOptions:\n  --release   Create optimized output instead of the default debug build.\n  --offline   Require cached dependencies and EffinDOM runtime inputs.";

const PACKAGE_USAGE: &str = "Package the current native FUI-RS application.\n\nUsage:\n  cargo fui package [--debug] [--offline]\n\nOutput:\n  macOS       DMG\n  Windows     MSIX\n  Linux       AppImage\n\nOptions:\n  --debug     Package development output instead of the default release build.\n  --offline   Require cached dependencies and EffinDOM runtime inputs.";

fn usage() -> Error {
    Error::Cli(USAGE.to_string())
}

fn command_or_help(
    arguments: &[String],
    help: &str,
    io: &CliIo<'_>,
    command: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if arguments.iter().any(|argument| is_help(argument)) {
        io.print(help);
        Ok(())
    } else {
        command()
    }
}

fn is_help(argument: &str) -> bool {
    matches!(argument, "help" | "--help" | "-h")
}

fn print_help(command: Option<&str>, io: &CliIo<'_>) -> Result<()> {
    let help = match command {
        None => USAGE,
        Some("new") => NEW_USAGE,
        Some("dev") => DEV_USAGE,
        Some("build") => BUILD_USAGE,
        Some("package") => PACKAGE_USAGE,
        Some(command) => {
            return Err(Error::Cli(format!(
                "unknown command {command:?}\n\n{USAGE}"
            )))
        }
    };
    io.print(help);
    Ok(())
}

fn new_command(cwd: &Path, arguments: &[String], io: &CliIo<'_>) -> Result<()> {
    let path = arguments
        .first()
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(usage)?;
    let target = option_value(arguments, "--target")?.unwrap_or("native");
    reject_options(arguments, &[path.as_str(), "--target", target])?;
    let destination = cwd.join(path);
    let project_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fui-app")
        .to_string();
    create_project(&NewProjectOptions {
        destination: destination.clone(),
        project_name,
        template: ProjectTemplate::parse(target)?,
    })?;
    io.print(&format!("Created {}", destination.display()));
    io.print(&format!("Run: cd {path} && cargo fui dev"));
    Ok(())
}

fn build_command(cwd: &Path, arguments: &[String], io: &CliIo<'_>) -> Result<()> {
    let options = build_options(cwd, arguments, BuildProfile::Debug)?;
    for output in build_project(&options)? {
        io.print(&format!("Built {}", output.path.display()));
    }
    Ok(())
}

fn dev_command(cwd: &Path, arguments: &[String], io: &CliIo<'_>) -> Result<()> {
    let options = build_options(cwd, arguments, BuildProfile::Debug)?;
    dev_project(&options, |message| io.print(message))
}

fn package_command(cwd: &Path, arguments: &[String], io: &CliIo<'_>) -> Result<()> {
    let options = build_options(cwd, arguments, BuildProfile::Release)?;
    let output = package_project(&options)?;
    io.print(&format!("Packaged {}", output.display()));
    Ok(())
}

fn build_options(cwd: &Path, arguments: &[String], default: BuildProfile) -> Result<BuildOptions> {
    let profile = if arguments.iter().any(|value| value == "--release") {
        BuildProfile::Release
    } else if arguments.iter().any(|value| value == "--debug") {
        BuildProfile::Debug
    } else {
        default
    };
    let offline = arguments.iter().any(|value| value == "--offline");
    reject_options(arguments, &["--release", "--debug", "--offline"])?;
    Ok(BuildOptions {
        project_root: discover_project(cwd)?,
        profile,
        offline,
    })
}

fn discover_project(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join("fui.toml").is_file() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(Error::ProjectNotFound(start.to_path_buf()));
        }
    }
}

fn option_value<'a>(arguments: &'a [String], name: &str) -> Result<Option<&'a str>> {
    let Some(index) = arguments.iter().position(|argument| argument == name) else {
        return Ok(None);
    };
    arguments
        .get(index + 1)
        .map(String::as_str)
        .map(Some)
        .ok_or_else(|| Error::Cli(format!("{name} requires a value")))
}

fn reject_options(arguments: &[String], allowed: &[&str]) -> Result<()> {
    if let Some(argument) = arguments
        .iter()
        .find(|argument| !allowed.contains(&argument.as_str()))
    {
        return Err(Error::Cli(format!("unexpected argument {argument:?}")));
    }
    Ok(())
}
