use std::ffi::OsString;

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
