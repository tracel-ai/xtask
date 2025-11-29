use tracel_xtask::prelude::*;

#[macros::declare_command_args(Target, None)]
struct MyCommandCmdArgs {}

pub fn handle_command(_args: MyCommandCmdArgs) -> anyhow::Result<()> {
    eprintln!("Hello from my-command");
    Ok(())
}
