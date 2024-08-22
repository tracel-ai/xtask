use tracel_xtask::prelude::*;

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

    // tests
    base_commands::test::handle_command(TestCmdArgs {
        target: target.clone(),
        exclude: exclude.clone(),
        only: only.clone(),
        threads: None,
        jobs: None,
        command: Some(TestSubCommand::All),
    })?;

    Ok(())
}
