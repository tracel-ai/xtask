# Breaking Changes

- `init_xtask` now takes an `XtaskArgs` parameter and the argument parsing is done with a dedicated function `parse_args<C: clap::Subcommand>`.
  You need to update the call to `init_xtask` into two function calls. This allows to mutate the command arguments before actually initializing
  xtask.

  Replace:

  ```rs
  let args = init_xtask::<Command>()?;
  ```

  With:

  ```rs
  let args = init_xtask::<Command>(parse_args::<Command>()?)?;
  ```

- The `execution environment` has been renamed to the `context` which is more accurate and broad. The flag `-E, --execution-environment` 
  is now `-c, --context`.

- All `handle_command` functions of base commands now take the `environment` and the `context` as parameters.

  Before:

  ```rs
  pub fn handle_command(args: TestCmdArgs) -> anyhow::Result<()> {}
  ```

  After:

  ```rs
  pub fn handle_command(args: TestCmdArgs, env: Environment, ctx: Context) -> anyhow::Result<()> {}
  ```

# New features

- Automatic sourcing of environment files containing environment variables given the value for the `-e,--environment` argument:
  - `.env` for any set environment,
  - `.env.{environment}` (example: `.env.dev`) for the non-sensitive configuration,
  - `.env.{environment}.secrets` (example `.env.dev.secrets`) for the sensitive configuration like password. These

- new command `docker` integrated with the automatic sourcing of environment variable files. It starts a docker compose stack with
  the naming scheme `docker-compose.{env}.yml`, `env` being the shorthand environment name.

- `TestCmdArgs` accepts new parameters `--force` and `--nocapture`.

- `BuildCmdArgs` accepts new parameter `--release`.
