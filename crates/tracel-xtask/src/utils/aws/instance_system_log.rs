use anyhow::Context as _;
use serde::Deserialize;
use std::{io::Write as _, thread, time::Duration};

use crate::utils::aws;

#[derive(Deserialize)]
struct ConsoleOutput {
    #[serde(rename = "Output")]
    output: Option<String>,
}

pub fn stream_system_log(region: &str, instance_id: &str) -> anyhow::Result<()> {
    let poll = Duration::from_secs(2);
    let mut printed_len: usize = 0;

    loop {
        let out = aws::cli::ec2_instance_get_console_output_json(region, instance_id)?;
        let parsed: ConsoleOutput =
            serde_json::from_str(&out).context("Parsing get-console-output JSON should succeed")?;
        let Some(text) = parsed.output.as_deref() else {
            thread::sleep(poll);
            continue;
        };

        if !text.is_empty() {
            if text.len() > printed_len {
                print!("{}", &text[printed_len..]);
                std::io::stdout().flush().ok();
                printed_len = text.len();
            } else if text.len() < printed_len {
                // reset because AWS returned a shorter buffer
                printed_len = 0;
            }
        }
        thread::sleep(poll);
    }
}
