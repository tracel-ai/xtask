use std::process::Command;

use crate::{
    context::Context,
    prelude::{anyhow::Context as _, Environment, EnvironmentName},
    utils::{self, aws},
};

const SSM_SESSION_DOC: &str = "Xtask-Host-InteractiveShell";

#[tracel_xtask_macros::declare_command_args(None, HostSubCommand)]
pub struct HostCmdArgs {}

impl Default for HostSubCommand {
    fn default() -> Self {
        HostSubCommand::Connect(HostConnectSubCmdArgs::default())
    }
}

#[derive(clap::Args, Clone, Default, PartialEq)]
pub struct HostConnectSubCmdArgs {
    /// Name of the host
    #[arg(long)]
    pub name: String,

    /// Region where the host lives
    #[arg(long)]
    pub region: String,

    /// Login user for the SSM interactive shell
    #[arg(long, default_value = "ubuntu")]
    pub user: String,
}

#[derive(clap::Args, Clone, Default, PartialEq)]
pub struct HostPrivateIpSubCmdArgs {
    /// Name of the host
    #[arg(long)]
    pub name: String,

    /// Region where the host lives
    #[arg(long)]
    pub region: String,
}

pub fn handle_command(args: HostCmdArgs, env: Environment, _ctx: Context) -> anyhow::Result<()> {
    if matches!(
        env.name,
        EnvironmentName::Development | EnvironmentName::Test
    ) {
        anyhow::bail!(
            "'database' command not supported for environment {env}, use local docker-compose or dev DB instead."
        );
    }

    match args.get_command() {
        HostSubCommand::Connect(connect_args) => connect(connect_args),
        HostSubCommand::PrivateIp(privateip_args) => private_ip(privateip_args, env),
    }
}

fn connect(args: HostConnectSubCmdArgs) -> anyhow::Result<()> {
    // 1) Resolve instance ID from EC2 using the Name tag
    let describe_output = Command::new("aws")
        .args([
            "ec2",
            "describe-instances",
            "--region",
            &args.region,
            "--filters",
            &format!("Name=tag:Name,Values={}", args.name),
            "Name=instance-state-name,Values=running",
            "--query",
            "Reservations[0].Instances[0].InstanceId",
            "--output",
            "text",
        ])
        .output()
        .with_context(|| {
            format!(
                "Describing database instance '{}' in region '{}' should succeed",
                args.name, args.region
            )
        })?;

    if !describe_output.status.success() {
        let stderr = String::from_utf8_lossy(&describe_output.stderr);
        anyhow::bail!(
            "Describing database instance '{}' in region '{}' should succeed, but AWS CLI exited with:\n{}",
             args.name,
            args.region,
            stderr
        );
    }

    let instance_id = String::from_utf8(describe_output.stdout)
        .context("Parsing database instance ID from AWS CLI output should succeed")?
        .trim()
        .to_string();

    if instance_id.is_empty() || instance_id == "None" {
        anyhow::bail!(
            "Finding a running database instance named '{}' in region '{}' should succeed, but none were found",
             args.name,
            args.region
        );
    }

    // 2) Ensure the SSM session document is present / up to date for this user
    aws::cli::ensure_ssm_document(SSM_SESSION_DOC, &args.region, &args.user)?;

    eprintln!(
        "🔌 Opening SSM session to database instance '{}' (id '{}') in region '{}' as user '{}'...",
        args.name, instance_id, args.region, args.user
    );

    let args_vec: Vec<&str> = vec![
        "ssm",
        "start-session",
        "--target",
        instance_id.as_str(),
        "--region",
        args.region.as_str(),
        "--document-name",
        SSM_SESSION_DOC,
    ];

    utils::process::run_process(
        "aws",
        &args_vec,
        None,
        None,
        "SSM session to database host should start successfully",
    )?;

    Ok(())
}

fn private_ip(args: HostPrivateIpSubCmdArgs, _env: Environment) -> anyhow::Result<()> {
    // 1) Ask AWS for the PrivateIpAddress of the running instance with this Name tag
    let describe_output = Command::new("aws")
        .args([
            "ec2",
            "describe-instances",
            "--region",
            &args.region,
            "--filters",
            &format!("Name=tag:Name,Values={}", args.name),
            "Name=instance-state-name,Values=running",
            "--query",
            "Reservations[0].Instances[0].PrivateIpAddress",
            "--output",
            "text",
        ])
        .output()
        .with_context(|| format!("Describing host instance '{}' should succeed", args.name))?;

    if !describe_output.status.success() {
        let stderr = String::from_utf8_lossy(&describe_output.stderr);
        anyhow::bail!(
            "Describing host instance '{}' should succeed, but AWS CLI exited with:\n{}",
            args.name,
            stderr
        );
    }

    // 2) Parse the private IP address
    let private_ip = String::from_utf8(describe_output.stdout)
        .context("Parsing host private IP from AWS CLI output should succeed")?
        .trim()
        .to_string();

    if private_ip.is_empty() || private_ip == "None" {
        anyhow::bail!(
            "Finding a running instance named '{}' should return a private IP address, but none were found",
            args.name,
        );
    }

    // 3) Print to stdout so this subcommand can be used in scripts
    println!("{private_ip}");

    Ok(())
}
