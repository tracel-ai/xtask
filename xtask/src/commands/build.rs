use strum::{Display, EnumIter, EnumString};
use tracel_xtask_commands::clap::ValueEnum;
use tracel_xtask_commands::commands::build::BuildCmdArgs;
use tracel_xtask_commands::commands::Target;
use tracel_xtask_commands::{anyhow, clap, declare_target};

declare_target!(BuildTarget, Frontend);


// #[tracel_xtask_macros::target()]
// pub enum BuildTarget {
//     /// Target the frontend.
//     Frontend,
// }

#[tracel_xtask_macros::command_args(BuildCmdArgs, BuildTarget)]
pub struct ExtendedBuildCmdArgs {}

// impl std::convert::TryInto<BuildCmdArgs> for ExtendedBuildCmdArgs {
//     type Error = anyhow::Error;
//     fn try_into(self) -> Result<BuildCmdArgs, Self::Error> {
//         let target = self.target.try_into()?;
//         Ok(BuildCmdArgs {
//             target,
//             exclude: self.exclude,
//             only: self.only,
//         })
//     }
// }

pub fn handle_command(args: ExtendedBuildCmdArgs) -> anyhow::Result<()> {
    match args.target {
        BuildTarget::Frontend => handle_frontend_target(args),
        _ => tracel_xtask_commands::commands::build::handle_command(args.try_into().unwrap()),
    }
}

fn handle_frontend_target(_args: ExtendedBuildCmdArgs) -> Result<(), anyhow::Error> {
    println!("Custom handling of extended target 'frontend'");
    Ok(())
}
