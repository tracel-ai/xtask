use anyhow::Context as _;
use serde::Deserialize;
use std::{io::Write as _, thread, time::Duration};

#[derive(Deserialize)]
struct ConsoleOutput {
    #[serde(rename = "Output")]
    output: Option<String>,
}

pub fn stream_system_log(region: &str, instance_id: &str) -> anyhow::Result<()> {
    let poll = Duration::from_secs(2);
    let mut printed_len: usize = 0;

    loop {
        let out = crate::utils::aws::cli::aws_cli_capture_stdout(
            vec![
                "ec2".into(),
                "get-console-output".into(),
                "--instance-id".into(),
                instance_id.into(),
                "--region".into(),
                region.into(),
                "--latest".into(),
                "--output".into(),
                "json".into(),
            ],
            "aws ec2 get-console-output should succeed",
            None,
            None,
        )?;

        let parsed: ConsoleOutput =
            serde_json::from_str(&out).context("Parsing get-console-output JSON should succeed")?;

        let Some(text) = parsed.output.as_deref() else {
            thread::sleep(poll);
            continue;
        };

        if text.is_empty() {
            thread::sleep(poll);
            continue;
        }

        if text.len() > printed_len {
            print!("{}", &text[printed_len..]);
            std::io::stdout().flush().ok();
            printed_len = text.len();
        } else if text.len() < printed_len {
            // reset because AWS returned a shorter buffer
            printed_len = 0;
        }

        thread::sleep(poll);
    }
}
