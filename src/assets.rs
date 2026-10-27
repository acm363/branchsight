pub struct Asset {
    pub body: &'static str,
    pub content_type: &'static str,
}

macro_rules! web_files {
    ($($name:literal),* $(,)?) => { &[$(($name, include_str!(concat!("web/", $name)))),*] };
}

const FILES: &[(&str, &str)] = web_files![
    "index.html",
    "style.css",
    "favicon.svg",
    "actions/commands.js",
    "actions/context-menu.js",
    "app.js",
    "changes/diffview.js",
    "changes/panel.js",
    "changes/patch.js",
    "changes/viewer.js",
    "header/branches.js",
    "header/projects.js",
    "header/settings.js",
    "history/graph.js",
    "history/labels.js",
    "history/list.js",
    "history/sync.js",
    "shared/api.js",
    "shared/dom.js",
    "shared/format.js",
    "shared/picker.js",
    "shared/prefs.js",
    "shared/state.js",
];

pub fn get(path: &str) -> Option<Asset> {
    let name = if path == "/" { "index.html" } else { path.strip_prefix('/')? };
    let (file, body) = FILES.iter().find(|(file, _)| *file == name)?;
    Some(Asset { body, content_type: content_type(file) })
}

fn content_type(file: &str) -> &'static str {
    match file.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
