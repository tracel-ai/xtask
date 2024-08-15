use tracel_xtask::prelude::*;

use super::build::{BuildTarget, ExtendedBuildCmdArgs};

pub fn handle_command() -> anyhow::Result<()> {
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
        base_commands::check::handle_command(CheckCmdArgs {
            target: target.clone(),
            exclude: exclude.clone(),
            only: only.clone(),
            command: Some(c.clone()),
        })
    })?;

    // build
    let build_target = BuildTarget::Workspace;
    super::build::handle_command(ExtendedBuildCmdArgs {
        target: build_target.clone(),
        exclude: exclude.clone(),
        only: only.clone(),
        debug: false,
    })?;

    // tests
    base_commands::test::handle_command(TestCmdArgs {
        target: target.clone(),
        exclude: exclude.clone(),
        only: only.clone(),
        threads: None,
        command: Some(TestSubCommand::All),
    })?;

    Ok(())
}
