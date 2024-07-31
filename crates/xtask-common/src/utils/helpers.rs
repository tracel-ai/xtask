use anyhow::Ok;

use crate::{endgroup, group, utils::process::run_process};

/// Allow to build additional crates outside the common build commands
pub fn additional_crates_build(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["build"];
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

/// Allow to unit test additional crates outside the common unit-tests commands
pub fn additional_crates_unit_tests(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["test"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Unit Tests: {} (with params: {})", *c, params_display);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            &format!("Unit test failed for {}", *c),
            true,
        )?;
        endgroup!();
        Ok(())
    })
}

/// Allow to integration test additional crates outside the common integration-tests commands
pub fn additional_crates_integration_tests(
    crates: Vec<&str>,
    params: Vec<&str>,
) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["test", "--test", "test_*"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!(
            "Integration Tests: {} (with params: {})",
            *c,
            params_display
        );
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        run_process(
            "cargo",
            &args,
            &format!("Integration test failed for {}", *c),
            true,
        )?;
        endgroup!();
        Ok(())
    })
}

/// Allow to build crate documentation additional crates outside the common doc commands
pub fn additional_crates_doc_build(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["doc", "--no-deps"];
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
