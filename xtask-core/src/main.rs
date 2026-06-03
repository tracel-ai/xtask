use tracel_xtask::prelude::*;

#[macros::base_commands(
    AwsContainer,
    AwsSecrets,
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
    GcpContainer,
    GcpSecrets,
    Host,
    Infra,
    Publish,
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
