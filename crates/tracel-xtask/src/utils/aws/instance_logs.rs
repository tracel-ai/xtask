use anyhow::Context as _;
use serde::Deserialize;

use crate::utils::aws::cli::aws_cli_capture_stdout;

#[derive(Debug, Deserialize)]
struct DescribeLogStreams {
    #[serde(rename = "logStreams")]
    log_streams: Vec<LogStream>,
}

#[derive(Debug, Deserialize)]
struct LogStream {
    #[serde(rename = "logStreamName")]
    log_stream_name: String,
}

/// Resolve the CloudWatch Logs stream name whose name *contains* `instance_id`.
/// Enforces the invariant: exactly one match must exist.
pub fn resolve_log_stream_name_containing_instance_id(
    region: &str,
    log_group: &str,
    instance_id: &str,
) -> anyhow::Result<String> {
    let json = aws_cli_capture_stdout(
        vec![
            "logs".into(),
            "describe-log-streams".into(),
            "--log-group-name".into(),
            log_group.into(),
            "--region".into(),
            region.into(),
            "--order-by".into(),
            "LastEventTime".into(),
            "--descending".into(),
            "--max-items".into(),
            "200".into(),
            "--output".into(),
            "json".into(),
        ],
        "aws logs describe-log-streams should succeed",
        None,
        None,
    )
    .map(|s| s.trim_end().to_string())?;

    let parsed: DescribeLogStreams =
        serde_json::from_str(&json).context("Parsing describe-log-streams JSON should succeed")?;

    let matches: Vec<&LogStream> = parsed
        .log_streams
        .iter()
        .filter(|s| s.log_stream_name.contains(instance_id))
        .collect();

    match matches.as_slice() {
        [] => anyhow::bail!(
            "A log stream name containing instance id '{instance_id}' should exist in log group '{log_group}'"
        ),
        [only] => Ok(only.log_stream_name.clone()),
        many => {
            let mut names: Vec<String> = many.iter().map(|s| s.log_stream_name.clone()).collect();
            names.sort();
            anyhow::bail!(
                "Exactly one log stream name should contain instance id '{instance_id}' in log group '{log_group}', found:\n- {}",
                names.join("\n- ")
            )
        }
    }
}
