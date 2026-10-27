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
    "actions.js",
    "api.js",
    "app.js",
    "branches.js",
    "diffview.js",
    "dom.js",
    "format.js",
    "graph.js",
    "history.js",
    "labels.js",
    "log.js",
    "panel.js",
    "patch.js",
    "picker.js",
    "prefs.js",
    "projects.js",
    "settings.js",
    "state.js",
    "viewer.js",
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
