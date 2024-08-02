use xtask_common::{
    anyhow::{self, Ok},
    clap,
    commands::Target,
};

#[xtask_macros::arguments(target)]
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
