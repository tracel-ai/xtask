use std::{collections::HashMap, path::Path};

use anyhow::Ok;

use crate::{endgroup, group, utils::process::run_process};

/// Allow to build additional crates outside the common build commands
pub fn custom_crates_build(
    crates: Vec<&str>,
    args: Vec<&str>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    group_msg: &str,
) -> anyhow::Result<()> {
    let mut base_args = vec!["build", "--color", "always"];
    base_args.extend(args);
    crates.iter().try_for_each(|c| {
        group!("Custom Build: {} ({})", *c, group_msg);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            envs.clone(),
            path,
            &format!("Custom build failed for {}", *c),
        )?;
        endgroup!();
        Ok(())
    })
}

/// Allow to check additional crates outside the common check commands
pub fn custom_crates_check(
    crates: Vec<&str>,
    args: Vec<&str>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    group_msg: &str,
) -> anyhow::Result<()> {
    let mut base_args = vec!["check", "--color", "always"];
    base_args.extend(args);
    crates.iter().try_for_each(|c| {
        group!("Custom Check: {} ({})", *c, group_msg);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            envs.clone(),
            path,
            &format!("Custom check failed for {}", *c),
        )?;
        endgroup!();
        Ok(())
    })
}

/// Allow to test additional crates with specific flags and config
pub fn custom_crates_tests(
    crates: Vec<&str>,
    args: Vec<&str>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    group_msg: &str,
) -> anyhow::Result<()> {
    let mut base_args = vec!["test", "--color", "always"];
    base_args.extend(args);
    crates.iter().try_for_each(|c| {
        group!("Custom Tests: {} ({})", *c, group_msg);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            envs.clone(),
            path,
            &format!("Custom test failed for {}", *c),
        )?;
        endgroup!();
        Ok(())
    })
}

/// Allow to build crate documentation additional crates outside the common doc commands
pub fn custom_crates_doc_build(
    crates: Vec<&str>,
    args: Vec<&str>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    group_msg: &str,
) -> anyhow::Result<()> {
    let mut base_args = vec!["doc", "--no-deps", "--color", "always"];
    base_args.extend(args);
    crates.iter().try_for_each(|c| {
        group!("Custom doc build: {} ({})", *c, group_msg);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            envs.clone(),
            path,
            &format!("Custom doc build failed for {}", *c),
        )?;
        endgroup!();
        Ok(())
    })
}
