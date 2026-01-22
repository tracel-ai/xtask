const SUBREPO_EMOJIS: &[(&str, &str)] = &[
    ("admin", "⚙️️️"),
    ("api", "🔌"),
    ("backend", "🧠"),
    ("cd", "🚀"),
    ("ci", "🤖"),
    ("console", "🖥 "),
    ("data", "🗄️"),
    ("db", "🗄️"),
    ("dev", "👨‍💻‍"),
    ("frontend", "🖥 "),
    ("infra", "🏗 "),
    ("ops", "🧰️"),
    ("monitor", "🚨"),
    ("platform", "🏗"),
    ("server", "🛰️"),
    ("tool", "🛠"),
    ("ui", "🎨"),
    ("web", "🌐"),
];

pub fn emoji_for_subrepo(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    for (needle, emoji) in SUBREPO_EMOJIS {
        if n.contains(needle) {
            return Some(*emoji);
        }
    }
    None
}

pub fn format_repo_label(name: &str) -> String {
    match emoji_for_subrepo(name) {
        Some(e) => format!("{e} {name}"),
        None => name.to_string(),
    }
}

pub fn print_run_header(label: &str) {
    let width = 78;
    let header = format!("{label}");
    let sep = "─".repeat(width);
    eprintln!("{sep}");
    eprintln!(" {header}");
    eprintln!("{sep}");
}
