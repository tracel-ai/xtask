use tracel_xtask::prelude::*;

#[derive(clap::Args)]
pub struct CustomArgs {}

#[macros::base_commands]
enum Command {
    /// A trivial consumer-defined command retained in every benchmark scenario.
    Custom(CustomArgs),
}

fn main() -> anyhow::Result<()> {
    let (args, environment) = init_xtask::<Command>(parse_args::<Command>()?)?;
    match args.command {
        Command::Custom(_) => Ok(()),
        #[allow(unreachable_patterns)]
        _ => dispatch_base_commands(args, environment),
    }
}
