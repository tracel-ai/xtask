use strum::{Display, EnumIter, EnumString};
use tracel_xtask_commands::commands::build::BuildCmdArgs;
use tracel_xtask_commands::commands::Target;
use tracel_xtask_commands::{anyhow, clap, declare_target};
use tracel_xtask_commands::clap::ValueEnum;

declare_target!(BuildTarget, Frontend);

#[tracel_xtask_macros::command_arguments(target::BuildTarget, exclude, only)]
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

impl std::convert::TryInto<BuildCmdArgs> for ExtendedBuildCmdArgs {
    type Error = anyhow::Error;
    fn try_into(self) -> Result<BuildCmdArgs, Self::Error> {
        let target = match self.target {
            BuildTarget::AllPackages => Target::AllPackages,
            BuildTarget::Crates => Target::Crates,
            BuildTarget::Examples => Target::Examples,
            BuildTarget::Workspace => Target::Workspace,
            BuildTarget::Frontend => return Err(anyhow::anyhow!("Frontend target is not supported.")),
        };
        Ok(BuildCmdArgs {
            target,
            exclude: self.exclude,
            only: self.only,
        })
    }
}

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
