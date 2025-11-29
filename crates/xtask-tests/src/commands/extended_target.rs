use tracel_xtask::prelude::*;

#[macros::extend_targets]
pub enum MyTarget {
    /// Target the frontend component of the monorepo.
    Frontend,
}

#[macros::declare_command_args(MyTarget, None)]
struct ExtendedTargetCmdArgs {}

pub fn handle_command(args: ExtendedTargetCmdArgs) -> anyhow::Result<()> {
    match args.target {
        // Default targets
        MyTarget::AllPackages => eprintln!("You chose the target: all-packages"),
        MyTarget::Crates => eprintln!("You chose the target: crates"),
        MyTarget::Examples => eprintln!("You chose the target: examples"),
        MyTarget::Workspace => eprintln!("You chose the target: workspace"),

        // Additional target
        MyTarget::Frontend => eprintln!("You chose the target: frontend"),
    };
    Ok(())
}
