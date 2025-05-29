use crate::prelude::{Context, Environment};

use super::{
    check::{CheckCmdArgs, CheckSubCommand},
    test::{TestCmdArgs, TestSubCommand},
    Target,
};

#[tracel_xtask_macros::declare_command_args(None, None)]
struct ValidateCmdArgs {}

pub fn handle_command(
    args: ValidateCmdArgs,
    env: Environment,
    context: Context,
) -> anyhow::Result<()> {
    let target = Target::Workspace;
    let exclude = vec![];
    let only = vec![];

    // checks
    [
        CheckSubCommand::Audit,
        CheckSubCommand::Format,
        CheckSubCommand::Lint,
        CheckSubCommand::Typos,
    ]
    .iter()
    .try_for_each(|c| {
        super::check::handle_command(
            CheckCmdArgs {
                target: target.clone(),
                exclude: exclude.clone(),
                only: only.clone(),
                command: Some(c.clone()),
                ignore_audit: args.ignore_audit,
            },
            env.clone(),
            context.clone(),
        )
    })?;

    // tests
    super::test::handle_command(
        TestCmdArgs {
            target: target.clone(),
            exclude: exclude.clone(),
            only: only.clone(),
            threads: None,
            jobs: None,
            command: Some(TestSubCommand::All),
            force: false,
            features: None,
            no_default_features: false,
        },
        env.clone(),
        context.clone(),
    )?;

    Ok(())
}
