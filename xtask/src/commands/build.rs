use tracel_xtask_commands::prelude::*;

// declare_target!(BuildTarget, Frontend);

#[macros::extend_targets]
pub enum BuildTarget {
    /// Target the frontend.
    Frontend,
}

#[macros::extend_command_args(BuildCmdArgs, BuildTarget)]
pub struct ExtendedBuildCmdArgs {}

// #[derive(clap::Args, Clone)]
// pub struct ExtendedBuildCmdArgs {
//     #[doc = r"The target on which executing the command."]
//     #[arg(short,long,value_enum,default_value_t = BuildTarget::Workspace)]
//     pub target: BuildTarget,
//     #[doc = r"Comma-separated list of excluded crates."]
//     #[arg(
//         short = 'x',
//         long,
//         value_name = "CRATE,CRATE,...",
//         value_delimiter = ',',
//         required = false
//     )]
//     pub exclude: Vec<String>,
//     #[doc = r"Comma-separated list of crates to include exclusively."]
//     #[arg(
//         short = 'n',
//         long,
//         value_name = "CRATE,CRATE,...",
//         value_delimiter = ',',
//         required = false
//     )]
//     pub only: Vec<String>,
// }
// impl std::convert::TryInto<BuildCmdArgs> for ExtendedBuildCmdArgs {
//     type Error = anyhow::Error;
//     fn try_into(self) -> Result<BuildCmdArgs, Self::Error> {
//         Ok(BuildCmdArgs {
//             target: self.target.try_into()?,
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
