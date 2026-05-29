use tracel_xtask::prelude::*;

#[macros::base_commands(
    AwsContainer,
    Build,
    Bump,
    Check,
    Clean,
    Compile,
    Coverage,
    Dependencies,
    Doc,
    DockerCompose,
    Fix,
    Host,
    Infra,
    Publish,
    Secrets,
    Test,
    Validate,
    Vulnerabilities
)]
enum Command {}

fn main() -> anyhow::Result<()> {
    let (args, environment) = init_xtask::<Command>(parse_args::<Command>()?)?;
    dispatch_base_commands(args, environment)?;
    Ok(())
}
