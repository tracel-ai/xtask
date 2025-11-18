use tracel_xtask::prelude::*;

#[macros::base_commands(
    Build,
    Bump,
    Check,
    Compile,
    Container,
    Coverage,
    Dependencies,
    Doc,
    DockerCompose,
    Fix,
    Publish,
    Secrets,
    Test,
    Validate,
    Vulnerabilities
)]
enum Command {}

fn main() -> anyhow::Result<()> {
    let args = init_xtask::<Command>(parse_args::<Command>()?)?;
    dispatch_base_commands(args)?;
    Ok(())
}
