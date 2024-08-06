use tracel_xtask_commands::{
    anyhow::{self, Ok},
    clap,
    commands::Target,
};

#[tracel_xtask_macros::command_args(Target)]
struct FooCmdArgs {}

pub fn handle_commands(args: FooCmdArgs) -> anyhow::Result<()> {
    match args.target {
        Target::AllPackages => println!("You chose the target: all-packages"),
        Target::Crates => println!("You chose the target: crates"),
        Target::Examples => println!("You chose the target: examples"),
        Target::Workspace => println!("You chose the target: workspace"),
    }
    Ok(())
}
