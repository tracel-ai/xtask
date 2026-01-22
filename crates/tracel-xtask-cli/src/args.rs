use std::ffi::{OsStr, OsString};

pub fn take_yes_flag(args: &mut Vec<OsString>) -> bool {
    let mut yes = false;
    args.retain(|a| {
        let keep = a != OsStr::new("-y") && a != OsStr::new("--yes");
        if !keep {
            yes = true;
        }
        keep
    });
    yes
}

pub fn take_subrepo_selector(args: &mut Vec<OsString>) -> Option<String> {
    let first = args.first()?.clone(); // own it; no borrow into args
    let s = first.to_string_lossy();
    if s.starts_with('-') {
        return None;
    }
    let rest = s.strip_prefix(':')?;
    if rest.is_empty() {
        return None;
    }
    args.remove(0);
    Some(rest.to_string())
}
