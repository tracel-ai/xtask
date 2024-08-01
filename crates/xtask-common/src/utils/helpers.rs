use anyhow::Ok;

use crate::{endgroup, group, utils::process::run_process};

/// Allow to build additional crates outside the common build commands
pub fn custom_crates_build(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let mut base_args = vec!["build", "--color", "always"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Custom Build: {}", *c);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process("cargo", &args, &format!("Custom build failed for {}", *c))?;
        endgroup!();
        Ok(())
    })
}

/// Allow to test additional crates with specific flags and config
pub fn custom_crates_tests(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let mut base_args = vec!["test", "--color", "always"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Custom Tests: {}", *c);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            &format!("Custom test failed for {}", *c),
        )?;
        endgroup!();
        Ok(())
    })
}

/// Allow to build crate documentation additional crates outside the common doc commands
pub fn custom_crates_doc_build(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let mut base_args = vec!["doc", "--no-deps", "--color", "always"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Custom doc build: {}", *c);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            &format!("Custom doc build failed for {}", *c),
        )?;
        endgroup!();
        Ok(())
    })
}
