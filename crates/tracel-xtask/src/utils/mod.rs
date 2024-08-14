use std::process::Command;

pub mod cargo;
pub mod helpers;
pub mod process;
pub mod prompt;
pub mod rustup;
pub mod time;
pub mod workspace;

pub fn get_command_line_from_command(command: &Command) -> String {
    let args: Vec<String> = command
        .get_args()
        .map(|arg| format!("\"{}\"", arg.to_string_lossy().into_owned()))
        .collect();
    format!(
        "{} {}",
        command.get_program().to_string_lossy(),
        args.join(" ")
    )
}
