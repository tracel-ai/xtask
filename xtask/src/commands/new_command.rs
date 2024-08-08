use tracel_xtask_commands::prelude::*;

#[macros::declare_command_args(Target)]
struct NewCommandCmdArgs {}

pub fn handle_commands(args: NewCommandCmdArgs) -> anyhow::Result<()> {
    match args.target {
        Target::AllPackages => println!("You chose the target: all-packages"),
        Target::Crates => println!("You chose the target: crates"),
        Target::Examples => println!("You chose the target: examples"),
        Target::Workspace => println!("You chose the target: workspace"),
    }
    Ok(())
}
