use std::process::Command as StdCommand;

use crate::prelude::{run_process, Context, Environment};

#[tracel_xtask_macros::declare_command_args(None, DockerSubCommand)]
pub struct DockerCmdArgs {}

pub fn handle_command(args: DockerCmdArgs, env: Environment, _ctx: Context) -> anyhow::Result<()> {
    match args.get_command() {
        DockerSubCommand::Up => up_docker_compose(&env, &args.project, args.build, args.services),
        DockerSubCommand::Down => down_docker_compose(&env, &args.project),
    }
}

fn get_config_filename(config: &str) -> String {
    format!("docker-compose.{config}.yml")
}

pub fn up_docker_compose(
    env: &Environment,
    project: &str,
    build: bool,
    services: Vec<String>,
) -> anyhow::Result<()> {
    let env_name = env.to_string();
    let dotenv_filepath = env.get_dotenv_filename();
    let project = format!("{project}-{env_name}");
    let config = get_config_filename(&env_name);
    let mut args = vec![
        "compose",
        "-f",
        &config,
        "--env-file",
        &dotenv_filepath,
        "-p",
        &project,
        "up",
        "-d",
    ];
    if build {
        args.extend(vec!["--build"]);
    }
    args.extend(services.iter().map(String::as_str));
    let result = run_process(
        "docker",
        &args,
        None,
        None,
        "Failed to execute 'docker compose' to start the container!",
    );
    if result.is_err() {
        run_process(
            "docker-compose",
            &args[1..],
            None,
            None,
            "Failed to execute 'docker compose' to start the container!",
        )
    } else {
        result
    }
}

pub fn down_docker_compose(env: &Environment, project: &str) -> anyhow::Result<()> {
    let env_name = env.to_string();
    let dotenv_filepath = env.get_dotenv_filename();
    let project = format!("{project}-{env_name}");
    let config = get_config_filename(&env_name);
    let args = vec![
        "compose",
        "-f",
        &config,
        "--env-file",
        &dotenv_filepath,
        "-p",
        &project,
        "down",
    ];
    let result = run_process(
        "docker",
        &args,
        None,
        None,
        "Failed to execute 'docker compose' to stop the container!",
    );
    if result.is_err() {
        run_process(
            "docker-compose",
            &args[1..],
            None,
            None,
            "Failed to execute 'docker compose' to stop the container!",
        )
    } else {
        result
    }
}

pub fn tail_container_command(container_name: &str) -> StdCommand {
    let mut command = StdCommand::new("docker");
    command.args(["logs", "-f", container_name]);
    command
}
