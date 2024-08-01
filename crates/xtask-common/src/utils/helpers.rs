use anyhow::Ok;

use crate::{endgroup, group, utils::process::run_process};

/// Allow to build additional crates outside the common build commands
pub fn additional_crates_build(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["build", "--color", "always"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Build: {} (with params: {})", *c, params_display);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process("cargo", &args, &format!("Build failed for {}", *c), true)?;
        endgroup!();
        Ok(())
    })
}

/// Allow to test additional crates with specific flags and config
pub fn additional_crates_tests(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["test", "--color", "always"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Additional Tests: {} (with params: {})", *c, params_display);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            &format!("Additional test failed for {}", *c),
            true,
        )?;
        endgroup!();
        Ok(())
    })
}

/// Allow to build crate documentation additional crates outside the common doc commands
pub fn additional_crates_doc_build(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["doc", "--no-deps", "--color", "always"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Doc Build: {} (with params: {})", *c, params_display);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            &format!("Doc build failed for {}", *c),
            true,
        )?;
        endgroup!();
        Ok(())
    })
}
