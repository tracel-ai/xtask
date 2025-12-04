use tracel_xtask::prelude::*;

#[macros::extend_targets]
pub enum MyTarget {
    /// Target the backend component of the monorepo, same thing as Workspace
    #[alias(Workspace)]
    Backend,
    /// Target the frontend component of the monorepo.
    Frontend,
}

#[macros::declare_command_args(MyTarget, None)]
struct ExtendedTargetAliasCmdArgs {}

pub fn handle_command(args: ExtendedTargetAliasCmdArgs) -> anyhow::Result<()> {
    match args.target.try_into().unwrap() {
        Target::AllPackages => eprintln!("You chose the target: all-packages"),
        Target::Crates => eprintln!("You chose the target: crates"),
        Target::Examples => eprintln!("You chose the target: examples"),
        Target::Workspace => eprintln!("You chose the target: workspace"),
    };
    Ok(())
}
