use std::process::Command;

use rstest::rstest;

#[rstest]
#[case::create_custom_command(&["my-command"], "Hello from my-command", true)]
#[case::create_custom_command_with_sub_commands_default_variant(&["my-command-with-sub-command"], "Execute Command 1 (default)", true)]
#[case::create_custom_command_with_sub_commands_variant_1(&["my-command-with-sub-command", "command1"], "Execute Command 1 (default)", true)]
#[case::create_custom_command_with_sub_commands_variant_2(&["my-command-with-sub-command", "command2"], "Execute Command 2", true)]
#[case::create_custom_command_with_extended_target_default(&["extended-target"], "You chose the target: workspace", true)]
#[case::create_custom_command_with_extended_target_all_packages(&["extended-target", "--target", "all-packages"], "You chose the target: all-packages", true)]
#[case::create_custom_command_with_extended_target_crates(&["extended-target", "--target", "crates"], "You chose the target: crates", true)]
#[case::create_custom_command_with_extended_target_examples(&["extended-target", "--target", "examples"], "You chose the target: examples", true)]
#[case::create_custom_command_with_extended_target_workspace(&["extended-target", "--target", "workspace"], "You chose the target: workspace", true)]
#[case::create_custom_command_with_extended_target_new_frontend_variant(&["extended-target", "--target", "frontend"], "You chose the target: frontend", true)]
#[case::extend_base_command_with_additional_command_args_debug_1(&["extended-build-args"], "debug disabled", true)]
#[case::extend_base_command_with_additional_command_args_debug_2(&["extended-build-args"], "debug disabled", true)]
#[case::extend_base_command_with_additional_command_args_debug_3(&["extended-test-args"], "debug disabled", true)]
#[case::extend_base_command_with_additional_command_args_debug_4(&["extended-test-args"], "debug disabled", true)]
#[case::extend_base_command_with_no_sub_commands_by_adding_sub_commands_default(&["extended-build-new-sub-commands"], "Executing build sub command 1", true)]
#[case::extend_base_command_with_no_sub_commands_by_adding_sub_commands_variant_1(&["extended-build-new-sub-commands", "command1"], "Executing build sub command 1", true)]
#[case::extend_base_command_with_no_sub_commands_by_adding_sub_commands_variant_2(&["extended-build-new-sub-commands", "command2"], "Executing build sub command 2", true)]
#[case::extend_base_command_with_sub_commands_by_adding_variants_default(&["extended-check-sub-commands"], "Executing all", true)]
#[case::extend_base_command_with_sub_commands_by_adding_variants_all(&["extended-check-sub-commands", "all"], "Executing all", true)]
#[case::extend_base_command_with_sub_commands_by_adding_variants_audit(&["extended-check-sub-commands", "audit"], "Executing audit", true)]
#[case::extend_base_command_with_sub_commands_by_adding_variants_format(&["extended-check-sub-commands", "format"], "Executing format", true)]
#[case::extend_base_command_with_sub_commands_by_adding_variants_lint(&["extended-check-sub-commands", "lint"], "Executing lint", true)]
#[case::extend_base_command_with_sub_commands_by_adding_variants_typos(&["extended-check-sub-commands", "typos"], "Executing typos", true)]
#[case::extend_base_command_with_sub_commands_by_adding_variants_new_variant(&["extended-check-sub-commands", "my-sub-command"], "Executing new subcommand", true)]
#[case::extend_base_command_advanced_example(&["extended-fix", "--target", "ci", "new-sub-command"], "Executing new subcommand on CI.", true)]
#[case::extend_base_command_advanced_example_default_target(&["extended-fix", "new-sub-command"], "Executing new subcommand on workspace.", true)]
#[case::cannot_execute_tests_in_production(&["-e", "prod", "test"], "Abort tests to avoid running them in production!", false)]
#[case::force_tests_execution_in_production(&["-e", "prod", "extended-test-args", "-f"], "Force running tests in production (--force argument is set)", true)]
fn test_xtask_example_status_success_and_returns_expected_output(
    #[case] cargo_args: &[&str],
    #[case] expected_output: String,
    #[case] success: bool,
) {
    let mut args = vec!["xtask"];
    args.extend(cargo_args);
    let output = Command::new("cargo")
        .args(args)
        .output()
        .expect("cargo process should start");
    let out = String::from_utf8_lossy(&output.stdout);
    println!("{out}");
    assert_eq!(output.status.success(), success);
    assert!(out.contains(&expected_output));
}
