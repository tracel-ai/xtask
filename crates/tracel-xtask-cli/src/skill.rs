const XTASK_AGENT_SKILL: &str = r#"# Tracel xtask agent skill

Use this guide when you are an automation or coding agent working in a repository
that uses Tracel xtask.

## Purpose

`xtask` is a repository task runner for Rust workspaces and mixed-language
repositories. The installed `xtask` binary is a transparent wrapper: it discovers
the repository layout, compiles the repository-local xtask crate with Cargo, and
forwards task arguments to that crate.

Think of the wrapper as the stable entrypoint and the repository-local xtask
crate as the project-specific implementation.

## Invocation grammar

```text
xtask [+nightly|+n] [:<subrepo>|:all] [<xtask args...>]
xtask +skill
xtask +update
```

- `xtask` with no arguments prints wrapper help.
- `xtask --help` forwards to the underlying repository xtask help.
- `xtask <command> --help` forwards command help to the selected xtask crate.
- `+skill` is handled by the wrapper and prints this agent guide.
- `+update` is handled by the wrapper and runs `cargo install tracel-xtask-cli`
  to update itself.
- `+nightly` and `+n` run the selected xtask through the nightly toolchain.

## Repository discovery

The wrapper starts from the current directory, finds the git root, and then
classifies the repository.

Standard repository:

- The git root is a Cargo workspace.
- One workspace member or excluded package has a package name starting with
  `xtask`, preferably exactly `xtask`.
- Commands run at the git root.

Monorepo:

- The git root is not itself an xtask workspace.
- Immediate child directories may be subrepos.
- Each subrepo is a Cargo workspace with its own xtask-like package.
- From inside a subrepo, commands run in that subrepo.
- From the monorepo root, commands prompt before running in every subrepo.
- `:all` runs in every subrepo without prompting.
- `:<subrepo>` selects one subrepo. Exact names, unambiguous prefixes, and
  shorthands are accepted. A shorthand is built from the first letter of each
  name segment, so `product-backend` can be selected as `:pb`.

## What the wrapper does before execution

- Sets `XTASK_CLI=1` for the repository-local xtask process.
- Sets `XTASK_MONOREPO=1` when running inside a selected monorepo subrepo.
- Uses a deterministic target directory under `target/` to keep wrapper builds
  isolated from normal project builds.
- If a monorepo has `Dependencies.toml` at the git root, synchronizes matching
  dependency declarations into selected subrepo `Cargo.toml` files before
  executing commands. See "Dependency synchronization" below before assuming
  what it will edit.
- Prints a summary after `:all` dispatch showing which subrepos succeeded or
  failed.

## Repository-local xtask model

A repository-local xtask crate usually depends on `tracel-xtask` and has a
small `main.rs`:

```rust
use tracel_xtask::prelude::*;

#[macros::base_commands(Build, Check, Fix, Test)]
enum Command {}

fn main() -> anyhow::Result<()> {
    let (args, environment) = init_xtask::<Command>(parse_args::<Command>()?)?;
    dispatch_base_commands(args, environment)?;
    Ok(())
}
```

Projects can add custom commands by adding variants to `Command`, defining a
command argument struct with the macros from `tracel_xtask::prelude::macros`,
and dispatching those variants before falling back to `dispatch_base_commands`.

## Common base commands

Base commands vary by repository, but common ones include:

- `build`: build the workspace or selected target.
- `check`: run checks such as formatting, linting, typos, audit, or all checks.
- `fix`: apply formatting, lint fixes, and other repair tasks.
- `test`: run unit, integration, doc, miri, or all tests.
- `doc`: build or test documentation.
- `coverage`: install or generate coverage data.
- `dependencies`: inspect dependency health, unused dependencies, or deny rules.
- `vulnerabilities`: run nightly sanitizer and cargo-careful checks.
- deployment-oriented commands such as `aws-container`, `gcp-container`,
  `image`, `infra`, `aws-secrets`, and `gcp-secrets` when enabled.

Always confirm available commands with `xtask --help` in the selected context.

## Testing model

The `test` base command separates Rust tests into unit and integration tests.
Use the xtask command instead of calling Cargo directly when the repository
provides it, because xtask also applies environment safety checks, target
selection, package filtering, Miri setup, and consistent output grouping.

- Unit tests are tests compiled with library, binary, and example targets. In
  Rust projects these are usually inline `#[cfg(test)]` modules inside `src/`
  files.
- Integration tests are crate-level test targets, usually files under a
  crate's `tests/` directory beside `src/`.
- `xtask test unit` runs unit tests only.
- `xtask test integration` runs integration tests only.
- `xtask test all` runs every test subcommand except `all` itself.
- `--target workspace` is the default and uses workspace-level Cargo test
  commands. `--target crates`, `--target examples`, and `--target all-packages`
  iterate through selected workspace members.
- `--only` and `--exclude` are useful when package-by-package targets are used;
  `--only` is ignored for the workspace target.
- `--test <name-or-pattern>` filters the Cargo test target or test name,
  depending on the test mode.
- `--miri` requires the nightly toolchain, so use `xtask +n test --miri ...`.
- Tests refuse to run in the production environment unless `--force` is passed.

## Environment management

An xtask environment is a named execution context plus a numeric index. It is
used by commands to choose deployment targets, dotenv files, cloud tags, image
tags, secret names, and any custom project behavior that depends on environment.

Global options belong before the command:

```text
xtask -e stag check all
xtask --context no-std build
xtask --enable-coverage test all
```

- `-e` / `--env_name` selects the environment. Common values include
  `dev`, `test`, `stag`, and `prod`. Long names are `development`, `test`,
  `staging`, and `production`; short names are `d`, `t`, `s`, and `p`.
- `-i` / `--env_index` selects an environment index from 1 to 255. Index `1`
  is implicit in the default display style, so `stag` means `stag1`. Indexes
  greater than `1` are appended, for example `stag2`.
- `-c` / `--context` passes an arbitrary context such as `std` or `no-std`.
- `--enable-coverage` sets Rust coverage environment variables before the task.

When `init_xtask` runs, it creates the `Environment` and loads dotenv files for
the selected environment from the repository-local xtask working directory. For
environment `stag`, index `1`, the attempted files are:

- `.env`
- `.env.stag`
- `.env.secrets`
- `.env.stag.secrets`
- `.env.infra`
- `.env.stag.infra`
- `.env.infra.secrets`
- `.env.stag.infra.secrets`

For indexed environments, the medium environment name includes the index when
the index is greater than `1`, so staging index `2` uses files such as
`.env.stag2` and `.env.stag2.secrets`.

Only files that exist are loaded. Loading uses `dotenvy::from_path`, so existing
process environment variables are preserved. If the same key appears in more
than one loaded file during normal initialization, the first loaded value wins
because it becomes an existing process variable for later files. Some utilities
may explicitly merge env files for a generated file; that merge path uses the
same file list but later files replace earlier values in the merged output.

## Dependency synchronization

In a monorepo, `Dependencies.toml` at the git root is a Cargo-manifest-shaped
source of truth for dependency specs. It must contain `[workspace.dependencies]`.
Before selected subrepo commands run, the wrapper applies matching entries from
that root table to the selected subrepo `Cargo.toml` files.

Important sync rules:

- Sync only touches dependencies already declared in a subrepo manifest; it does
  not add every root dependency everywhere.
- It checks `[workspace.dependencies]`, top-level `dependencies`,
  `dev-dependencies`, `build-dependencies`, and target-specific dependency
  tables.
- `version`, `path`, `git`, `tag`, `rev`, `branch`, and `package` come from the
  root spec when present. Relative `path` values are rebased for each subrepo.
- Root `features` and `default-features` are authoritative only when they are
  declared in `Dependencies.toml`.
- If a dependency in `Dependencies.toml` does not declare `features`, the sync
  does not overwrite or remove the feature selection already present in a
  subrepo workspace. The same preservation rule applies to `default-features`.
- A subrepo dependency written as `{ workspace = true }` is left untouched.
- If a subrepo declares a dependency that is missing from root
  `[workspace.dependencies]`, the wrapper reports a warning for that manifest
  and table.

## Agent workflow

1. Run `xtask +skill` if you need this guide.
2. Run `xtask` to understand wrapper context and subrepo discovery.
3. Run `xtask --help` or `xtask :<subrepo> --help` to see project commands.
4. Prefer explicit selectors in monorepos:
   - `xtask :api check all`
   - `xtask :frontend test all`
   - `xtask :all check all`
5. Use `+n` for commands that require nightly:
   - `xtask +n test unit --miri`
   - `xtask +n test --miri`
   - `xtask +n vulnerabilities all`
6. Be aware that `fix`, dependency sync, generated docs, coverage, and
   deployment commands may modify files or external state.
7. For CI-like validation, prefer focused commands first, then broaden:
   - `xtask check format`
   - `xtask check lint`
   - `xtask test unit`
   - `xtask test integration`
   - `xtask test all`
   - `xtask :all check all`

## Practical rules

- Do not assume a repository is standard or monorepo; ask the wrapper.
- Do not call Cargo subcommands directly when an xtask command exists for the
  same workflow, because project-specific setup may live in xtask.
- Put wrapper selectors and toolchain overrides before repository-local args.
- Put repository-local global options before the repository-local command.
- Treat `--help` as transparent help for the selected underlying xtask, not as
  wrapper help.
- Use `:all` deliberately; it can run many commands and may edit many subrepos.
"#;

pub(crate) fn text() -> &'static str {
    XTASK_AGENT_SKILL.trim()
}

pub(crate) fn print() {
    println!("{}", text());
}
