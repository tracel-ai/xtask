use std::process::Command;

use anyhow::{anyhow, Ok};

use crate::{endgroup, group};


/// Allow to build additional crates outside the common build commands
pub fn additional_crates_build(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["build"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Build: {} (with params: {})", *c, params_display);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        let status = Command::new("cargo")
            .args(args)
            .status()
            .map_err(|e| anyhow!("Failed to execute cargo build: {}", e))?;
        if !status.success() {
            return Err(anyhow!("Build failed for {}", *c));
        }
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
        let status = Command::new("cargo")
            .args(args)
            .status()
            .map_err(|e| anyhow!("Failed to execute cargo test: {}", e))?;
        if !status.success() {
            return Err(anyhow!("Unit test failed for {}", *c));
        }
        endgroup!();
        Ok(())
    })
}

/// Allow to integration test additional crates outside the common integration-tests commands
pub fn additional_crates_integration_tests(crates: Vec<&str>, params: Vec<&str>) -> anyhow::Result<()> {
    let params_display = params.join(", ");
    let mut base_args = vec!["test", "--test", "test_*"];
    base_args.extend(params);
    crates.iter().try_for_each(|c| {
        group!("Integration Tests: {} (with params: {})", *c, params_display);
        let mut args = base_args.clone();
        args.extend(vec!["-p", *c]);
        let status = Command::new("cargo")
            .args(args)
            .status()
            .map_err(|e| anyhow!("Failed to execute cargo test: {}", e))?;
        if !status.success() {
            return Err(anyhow!("Integration test failed for {}", *c));
        }
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
        let status = Command::new("cargo")
            .args(args)
            .status()
            .map_err(|e| anyhow!("Failed to execute cargo doc: {}", e))?;
        if !status.success() {
            return Err(anyhow!("Doc build failed for {}", *c));
        }
        endgroup!();
        Ok(())
    })
}
