use cargo_fui::{run_cli, CliIo};
use std::{cell::RefCell, path::PathBuf};

fn run(arguments: &[&str]) -> cargo_fui::Result<String> {
    let output = RefCell::new(Vec::<String>::new());
    let io = CliIo::new(|message| output.borrow_mut().push(message.to_string()));
    run_cli(
        arguments.iter().map(|argument| argument.to_string()),
        Ok(PathBuf::from(".")),
        &io,
    )?;
    drop(io);
    Ok(output.into_inner().join("\n"))
}
#[test]
fn global_help_orients_first_time_users() {
    let output = run(&["fui", "--help"]).expect("global help");

    assert!(output.contains("project, development, build, and packaging"));
    assert!(output.contains("cargo fui new <path>"));
    assert!(output.contains("cargo fui help <command>"));
}

#[test]
fn new_help_explains_every_project_shape_without_creating_a_project() {
    let direct = run(&["fui", "new", "--help"]).expect("new help");
    let delegated = run(&["fui", "help", "new"]).expect("delegated new help");

    assert_eq!(direct, delegated);
    assert!(direct.contains("native      Native macOS, Windows, or Linux"));
    assert!(direct.contains("web         Browser/WebAssembly"));
    assert!(direct.contains("universal   Shared retained UI"));
    assert!(direct.contains("default target is native"));
}

#[test]
fn project_commands_show_help_without_requiring_a_project() {
    for command in ["dev", "build", "package"] {
        let output = run(&["fui", command, "--help"]).expect("command help");
        assert!(output.contains(&format!("cargo fui {command}")));
    }
}
