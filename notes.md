# v5

`tracel-xtask`, `tracel-xtask-macros`, and `tracel-xtask-utils` are released as 5.0.0.

## Breaking changes

- Base commands are selected with `tracel-xtask` Cargo features. The `#[macros::base_commands]` attribute no longer
  accepts a command list.

  ```toml
  tracel-xtask = { version = "5", features = ["build", "check", "fix", "test"] }
  ```

  ```rust
  #[macros::base_commands]
  enum Command {}
  ```

- Default features enable no base commands. Enable `all` to generate all 21 base commands, or select individual
  kebab-case command features to compile only the commands and third-party dependencies the repository uses.
- Command modules, argument types, and optional prelude exports are feature-gated. Code that imports or extends a
  base command must enable that command's feature.
- `tracel-xtask-utils` also defaults to no features. Custom commands can enable utilities through the top-level
  `utils-<feature>` passthroughs, `utils-aws` or `utils-gcp` provider umbrellas, or `utils-all`.

## New features

- The command features are `aws-container`, `aws-secrets`, `build`, `bump`, `check`, `clean`, `compile`, `coverage`,
  `dependencies`, `doc`, `docker-compose`, `fix`, `gcp-container`, `gcp-secrets`, `host`, `image`, `infra`,
  `publish`, `test`, `validate`, and `vulnerabilities`.
- `validate` can be selected without exposing the standalone `check` and `test` commands.
- AWS and GCP utility modules have granular features, allowing cloud commands and custom commands to avoid provider
  utilities and third-party crates they do not use.

# Previous release notes

## Breaking Changes

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

## New features

- Automatic sourcing of environment files containing environment variables given the value for the `-e,--environment` argument:
  - `.env` for any set environment,
  - `.env.{environment}` (example: `.env.dev`) for the non-sensitive configuration,
  - `.env.{environment}.secrets` (example `.env.dev.secrets`) for the sensitive configuration like password. These

- new command `docker` integrated with the automatic sourcing of environment variable files. It starts a docker compose stack with
  the naming scheme `docker-compose.{env}.yml`, `env` being the shorthand environment name.

- `TestCmdArgs` accepts new parameters `--force` and `--nocapture`.

- `BuildCmdArgs` accepts new parameter `--release`.
