use std::ffi::OsString;

pub fn take_subrepo_selectors(args: &mut Vec<OsString>) -> Vec<String> {
    let mut selectors = Vec::new();

    while let Some(first) = args.first() {
        let selector = first.to_string_lossy();
        let Some(selector) = selector.strip_prefix(':').filter(|value| !value.is_empty()) else {
            break;
        };

        selectors.push(selector.to_string());
        args.remove(0);
    }

    selectors
}
