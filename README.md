# Tracel Xtask Commands

This repository holds all the wrappers around `cargo` and other tools that we use in all of our repositories to manage
the code quality, checks and tests easily. Those wrappers are implemented as [xtask commands][1] using [clap][2] and they
are designed to work with cargo workspaces.

By centralizing all of our redundant commands we can save a big amount of code duplication, boilerplate and considerably
lower the burden maintenance. It also provides a unified interface across all of our repositories.

These commands are not specific to Tracel repositories and they should be pretty much usable in any Rust repositories as
well as other repositories where Rust is not necessarily the only language. Indeed the commands can be easily extended
by following some patterns mentioned in this README.

## Getting Started

To get started create an `xtask` crate in your repository and add both `xtask-common` and `xtask-macros` crates as dependencies.

```sh
cargo new xtask --bin
cd xtask
echo '[dependencies]\nxtask-common = "*"\nxtask-macros = "*"' >> Cargo.toml
cargo build
```

In the `main.rs` file declare a `Command` struct and select the commands you want to use using `tracel_xtask_macros::commands` macro:

```rust
use tracel_xtask_commands::{anyhow, clap, commands::*, init_xtask};

#[tracel_xtask_macros::commands(
    Build,
    Check,
    Fix,
    Publish,
    Test,
)]
pub enum Command {}

fn main() -> anyhow::Result<()> {
    let args = init_xtask::<Command>()?;
    match args.command {
        Command::Build(args) => build::handle_command(args),
        Command::Check(args) => check::handle_command(args),
        Command::Fix(args) => fix::handle_command(args, None),
        Command::Publish(args) => publish::handle_command(args),
        Command::Test(args) => test::handle_command(args),
    }?;
    Ok(())
}
```

Then you should be able to display the main help screen which lists the available commands:

```sh
cargo xtask --help
```

**Pro-tip:** create an alias in your shell of choice to map `cargo xtask` to something easy to type like `cx`.

For bash:

```bash
nano ~/.bashrc

# add this to the file
alias cx='cargo xtask'

# save and source the file or restart the shell session
source ~/.bashrc
```

For fish:

```fish
nano ~/.config/fish/config.fish


# add this to the file
alias cx='cargo xtask'

# save and source the file or restart the shell session
source ~/.config/fish/config.fish
```

For powershell:

```powershell
notepad $PROFILE

# add this at the end of file
function cx {
    cargo xtask $args
}

# save and quit then open a new powershell terminal
```

## Interface overview and conventions

### Repository structure

All our repositories follow the same directory hierarchy:
- a `crates` directory which contains all the crates of the workspace
- a `examples` directory which holds all the examples crates
- a `xtask` directory which is binary crate of the CLI using `xtask-common`

### Integration tests

[Integration tests][3] are tests contained in a `tests` directory. `xtask-common` expects all the integration file names to be
prefixed with `test_`. If a file in the `tests` directory of a crate does not start with `test_` then it will be ignored when
running the `cargo xtask test integration` command.

### Target

There are 4 default targets provided by the xtask-common command line interface:
- `workspace` which targets the whole workspace
- `crates` are all the binary crates and library crates
- `examples` are all the example crates
- `all-packages` are both `crates` and `examples` targets

`workspace` and `all-packages` are different because `workspace` uses the `--workspace` flag of cargo whereas `all-packages`
relies on `crates` and `examples` targets which use the `--package` flag.

Here are some examples:

```sh
# run all the crates tests
cargo xtask test --target crates all
# check format for examples, binaries and libs
cargo xtask check --target all-packages unit
# build the workspace
cargo xtask build --target workspace
# workspace is the default target so this has the same effect
cargo xtask build
```

### Global options

The following options are global and must be use before the actual command on the command line, for instance:

```sh
cargo xtask -e no-std build
```

`-e` or `--execution-environment` does not do anything per se in the `xtask-common` commands, it is a flag whose only goal is
to inform your custom commands or dispatch about the targeted execution environment which can be `std` or `no-std`.


Another global option is `-c` or `--enabled-coverage** which setup Rust toolchain to generate coverage information.

## Create a custom command

To add specific new commands, first add new variants to the `Command` enum:

```rust
mod commands;

use tracel_xtask_commands::{anyhow, clap, commands::*, init_xtask};

#[tracel_xtask_macros::commands(
    Build,
    Check,
    Fix,
    Publish,
    Test,
)]
pub enum Command {
    /// This is a specific command to this repository
    MyCommand(commands::mycommand::MyCommandCmdArgs),
}
```

Then add the corresponding arm to the `args.command` match expression:

```rust
fn main() -> anyhow::Result<()> {
    let args = init_xtask::<Command>()?;
    match args.command {
        Command::Build(args) => build::handle_command(args),
        Command::Check(args) => check::handle_command(args),
        Command::Fix(args) => fix::handle_command(args, None),
        Command::Publish(args) => publish::handle_command(args),
        Command::Test(args) => test::handle_command(args),

        // This is a new command implmemented in this repository
        Command::MyCommand(args) => commands::mycommand::handle_command(args),
    }?;
    Ok(())
}
```

As you may have notice we organize the commands in a `commands` module. So to implement the new command, create a file
`xtask/src/commands/mycommand.rs` as well as the corresponding `mod.rs` file to declare the module contents.

In `mycommand.rs` we will define the `MyCommandCmdArgs` struct that holds the accepted arguments for this command,
add we will implement the function `handle_command` which executes the `my-command` command (on the command line `MyCommand`
becomes `my-command`).

Note that `xtask-macros` provides a macro to easily add common arguments to the argument struct like the `--target` as well as the
exclusion of crates with `--exclusion` etc... In this case we will just add the `--target` argument:

```rust
use tracel_xtask_commands::{commands::Target, anyhow::{self, Ok}, clap};

#[tracel_xtask_macros::arguments(target::Target)]
struct MyCommandCmdArgs {}
```

You can also add any custom arguments or subcommands inside of the `MyCommandCmdArgs` struct.

Now the `handle_command` function, we will make it output the chosen target:

```rust
pub fn handle_commands(args: MyCommandCmdArgs)  -> anyhow::Result<()> {
    match args.target {
        Target::AllPackages => println!("You chose the target: all-packages"),
        Target::Crates => println!("You chose the target: crates"),
        Target::Examples => println!("You chose the target: examples"),
        Target::Workspace => println!("You chose the target: workspace"),
    }
    Ok(())
}
```

You can now test your new command with:

```sh
cargo xtask my-command --target all-packages
```

## Extend an existing xtask-common command

Extending an existing `xtask-common` command is pretty easy and relies on the same principles described in the `Create a custom command` section.

To extend the `build` command for instance we can just create a custom command called `build` following the previous section.

Then inside the `handle_command` function we can call the implement provided by `xtask-common` like so:

```rust
pub fn handle_commands(mut args: tracel_xtask_commands::commands::build::BuildCmdArgs)  -> anyhow::Result<()> {
    // do some stuff before like tweaking the arguments or performinig setup
    tracel_xtask_commands::commands::build::handle_command(args)?;
    // do some stuff after like cleaning state etc...
    Ok(())
}
```

Note that we pass the arguments as mutable so we can tweak them before actually calling the `xtask-common` implementation.
For instance we could tweak the `--exclude` or `--only` argument values.

## Custom builds and tests

`xtask-common` provides helper functions to easily execute custom builds or tests with specific features or build target (do not confuse
Rust build targets which is an argument of the `cargo build` command with the xtask target we introduced here).

For instance we can extend the `build` command using the `Extend an existing xtask-common command` section and use the helper function to
build additional crates with custom features or build targets:

```rust
pub fn handle_commands(mut args: tracel_xtask_commands::commands::build::BuildCmdArgs)  -> anyhow::Result<()> {
    // regular execution of the build command
    tracel_xtask_commands::commands::build::handle_command(args)?;

    // additional crate builds
    // build 'my-crate' with all the features
    tracel_xtask_commands::utils::helpers::custom_crates_build(vec!["my-crate"], vec!["--all-features"])?;
    // build 'my-crate' with specific features
    tracel_xtask_commands::utils::helpers::custom_crates_build(vec!["my-crate"], vec!["--features", "myfeature1,myfeature2"])?;
    // build 'my-crate' with a different target than the default one
    tracel_xtask_commands::utils::helpers::custom_crates_build(vec!["my-crate"], vec!["--target", "thumbv7m-none-eabi"])?;
    Ok(())
}
```

## Enable and generate coverage information

Here is a example GitHub job which shows how to setup coverage, enable it and upload coverage information to codecov:

```yaml
env:
  GRCOV_LINK: "https://github.com/mozilla/grcov/releases/download"
  GRCOV_VERSION: "0.8.19"

jobs:
  my-job:
    runs-on: ubuntu-22.04
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      - name: install rust
        uses: dtolnay/rust-toolchain@master
        with:
          components: rustfmt, clippy
          toolchain: stable
      - name: Install grcov
        shell: bash
        run: |
          curl -L "$GRCOV_LINK/v$GRCOV_VERSION/grcov-x86_64-unknown-linux-musl.tar.bz2" |
          tar xj -C $HOME/.cargo/bin
          cargo xtask coverage install
      - name: Build
        shell: bash
        run: cargo xtask build
      - name: Tests
        shell: bash
        run: cargo xtask --enable-coverage test all
      - name: Generate lcov.info
        shell: bash
        # /* is to exclude std library code coverage from analysis
        run: cargo xtask coverage generate --ignore "/*,xtask/*,examples/*"
      - name: Codecov upload lcov.info
        uses: codecov/codecov-action@v4
        with:
          files: lcov.info
          token: ${{ secrets.CODECOV_TOKEN }}
```

## Special command 'validate'

The command `Validate` can been added via the macro `tracel_xtask_macros::commands`, this command has no implementations in `xtask_common` though.

This is a special command that you can implement in your repository to perform all the checks and tests needed to validate your code base.

Here is a simple example to perform all checks, build and test:

```rust
pub fn handle_command() -> anyhow::Result<()> {
    let target = tracel_xtask_commands::commands::Target::Workspace;
    let exclude = vec![];
    let only = vec![];
    // checks
    [
        tracel_xtask_commands::commands::check::CheckCommand::Audit,
        tracel_xtask_commands::commands::check::CheckCommand::Format,
        tracel_xtask_commands::commands::check::CheckCommand::Lint,
        tracel_xtask_commands::commands::check::CheckCommand::Typos,
    ]
    .iter()
    .try_for_each(|c| {
        tracel_xtask_commands::commands::check::handle_command(tracel_xtask_commands::commands::check::CheckCmdArgs {
            target: target.clone(),
            exclude: exclude.clone(),
            only: only.clone(),
            command: c.clone(),
        })
    })?;

    // build
    tracel_xtask_commands::commands::build::handle_command(
        tracel_xtask_commands::commands::build::BuildCmdArgs {
            target: target.clone(),
            exclude: exclude.clone(),
            only: only.clone(),
        },
    )?;

    // tests
    tracel_xtask_commands::commands::test::handle_command(
        tracel_xtask_commands::commands::test::TestCmdArgs {
            target: target.clone(),
            exclude: exclude.clone(),
            only: only.clone(),
            command: tracel_xtask_commands::commands::test::TestCommand::All,
        },
    )?;
}
```

## Extend targets

TODO: will be available in a later version

## Available xtask-common commands

### Check and Fix

`check` and `fix` contains the same subcommands to audit, format, lint or proofread a code base.

While the `check` command only reports issues the `fix` command attempt to fix them as they are encountered.

The `check` and `fix` commands are designed to help you maintain code quality during development.
They run various checks and fix issues, ensuring that your code is clean and follows best practices.

Each test can be executed separately or all of them can be executed sequentially using `all`.

Usage to lint the code base:
```sh
cargo xtask check lint
cargo xtask fix lint
cargo xtask fix all
```

### Running Tests

Testing is a crucial part of development, and the `test` command is designed to make this process easy.

This commands makes the distinction between unit tests and integrations tests. [Unit tests][4] are inline tests under the
`src` directory of a crate. [Integration tests][3] are files starting with `test_` under the `tests` directory of a crate.

Usage:
```sh
# execute workspace unit tests
cargo xtask test unit
# execute workspace integration tests
cargo xtask test integration
# execute workspace both unit tests and integration tests
cargo xtask test all
```

Note that documentation tests support is under the `doc` command.

### Documentation

Command to build and test the documentation in a workspace.

### Bumping Versions

This is a command reserved for repository maintainers.

The `bump` command is used to update the version numbers of all crates in the repository.
This is particularly useful when you're preparing for a new release and need to ensure that all crates have the correct version.

You can bump the version by major, minor, or patch levels, depending on the changes made.
For example, if you’ve made breaking changes, you should bump the major version.
For new features that are backwards compatible, bump the minor version.
For bug fixes, bump the patch version.

Usage:
```sh
cargo xtask bump <COMMAND>
```

### Publishing Crates

This is a command reserved for repository maintainers and is mainly used by the `publish` workflow.

This command automates the process of publishing crates to `crates.io`, the Rust package registry.
By specifying the name of the crate, `xtask` handles the publication process, ensuring that the crate is available for others to use.

Usage:
```sh
cargo xtask publish <NAME>
```

### Coverage

This command provide a subcommands to install the necessary dependencies for performing code coverage and a subcommand to generate the
coverage info file that can then be uploaded to codecov for instance. See dedicated section `Enable and generate coverage information`.

### Dependencies

Various additional commands about dependencies.

`deny` make sure that all dependencies meet requirements using [cargo-deny][5].

`unused` detects dependencies in the workspace that are not in ussed.

### Vulnerabilities

This command make it easier to execute sanitizers as described in [the Rust unstable book][6].

These sanitizers require a nightly toolchain.

```
Run the specified vulnerability check locally. These commands must be called with 'cargo +nightly'

Usage: xtask vulnerabilities <COMMAND>

Commands:
  all                            Run all most useful vulnerability checks
  address-sanitizer              Run Address sanitizer (memory error detector)
  control-flow-integrity         Run LLVM Control Flow Integrity (CFI) (provides forward-edge control flow protection)
  hw-address-sanitizer           Run newer variant of Address sanitizer (memory error detector similar to AddressSanitizer, but based on partial hardware assistance)
  kernel-control-flow-integrity  Run Kernel LLVM Control Flow Integrity (KCFI) (provides forward-edge control flow protection for operating systems kerneljs)
  leak-sanitizer                 Run Leak sanitizer (run-time memory leak detector)
  memory-sanitizer               Run memory sanitizer (detector of uninitialized reads)
  mem-tag-sanitizer              Run another address sanitizer (like AddressSanitizer and HardwareAddressSanitizer but with lower overhead suitable for use as hardening for production binaries)
  nightly-checks                 Run nightly-only checks through cargo-careful `<https://crates.io/crates/cargo-careful>`
  safe-stack                     Run SafeStack check (provides backward-edge control flow protection by separating stack into safe and unsafe regions)
  shadow-call-stack              Run ShadowCall check (provides backward-edge control flow protection - aarch64 only)
  thread-sanitizer               Run Thread sanitizer (data race detector)
  help                           Print this message or the help of the given subcommand(s)
```

[1]: https://github.com/matklad/cargo-xtask
[2]: https://github.com/clap-rs/clap
[3]: https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests
[4]: https://doc.rust-lang.org/book/ch11-03-test-organization.html#unit-tests
[5]: https://embarkstudios.github.io/cargo-deny/
[6]: https://doc.rust-lang.org/beta/unstable-book/compiler-flags/sanitizer.html
