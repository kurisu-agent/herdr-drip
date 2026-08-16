//! DRIP ADDITION. The plugin's config file — one file, both surfaces.
//!
//! Upstream had no settings at all: the status vocabulary was a `const` and
//! every runtime knob (`C` closed, `/` filter, `g` scope, `o` sort) died with
//! the process, which for a board you open and quit means it resets every
//! time. What we needed persisted is small — which statuses there are, in what
//! order, and whether closed ones show — and it is the same question the
//! sidebar rail answers, so it must not be answered twice.
//!
//! Where. `$HERDR_PLUGIN_CONFIG_DIR/config.json`, which herdr creates and
//! exports for every plugin command and plugin pane (`plugin_path_env`). There
//! is no manifest settings schema and no herdr-managed plugin storage API in
//! v1, so a file in the directory herdr already hands us is the blessed place.
//! With that variable unset there is no config and the defaults stand; the
//! path is not reconstructed from herdr's own scheme, which is hashed and not
//! ours to reproduce. HERDR_DRIP_BEADS_CONFIG names the file outright.
//!
//! What, and how the two surfaces read it:
//!
//! ```json
//! {
//!   "statuses": ["blocked", "in_progress", "open"],
//!   "show_closed": false
//! }
//! ```
//!
//!   - `statuses` — the vocabulary, in the order you want it. The BOARD groups
//!     and columns in this order (a status not listed but present on a bead is
//!     still appended, so this orders the board rather than censoring it); the
//!     RAIL treats it as a filter, since a rail that is five rows tall is
//!     nothing but a filter, and keeps its own worst-first order.
//!   - `show_closed` — the board's starting state for `C`, and whether the
//!     rail may show closed beads at all.
//!
//! Env wins over file, which is the layering the rail's other knobs already
//! have and what lets a nix module set these without writing a file:
//! `HERDR_DRIP_BEADS_STATUSES` (comma-separated) and
//! `HERDR_DRIP_BEADS_SHOW_CLOSED` (1/true/yes).
//!
//! Every value is read leniently: a malformed file, or a key of the wrong
//! type, leaves that key at its default rather than failing to start. A board
//! that will not open because its config has a typo in it is a worse outcome
//! than a board that opens in the default order, and there is no useful place
//! to put the error — this is read before there is a TUI to draw it in.

use std::sync::OnceLock;

/// The status vocabulary in board order, when nothing says otherwise.
/// Upstream's `STATUS_ORDER`, which mirrors bd 1.1.0's built-ins.
pub const DEFAULT_STATUSES: &[&str] = &[
    "open",
    "in_progress",
    "blocked",
    "hooked",
    "deferred",
    "pinned",
    "closed",
];

pub struct Config {
    pub statuses: Vec<String>,
    pub show_closed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            statuses: DEFAULT_STATUSES.iter().map(|s| s.to_string()).collect(),
            show_closed: false,
        }
    }
}

/// The config, read once per process. Nothing rereads it: a board is opened,
/// used and quit, and `r` refreshes beads rather than settings.
pub fn get() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(load)
}

fn config_path() -> Option<std::path::PathBuf> {
    if let Some(explicit) = non_empty("HERDR_DRIP_BEADS_CONFIG") {
        return Some(std::path::PathBuf::from(explicit));
    }
    non_empty("HERDR_PLUGIN_CONFIG_DIR").map(|dir| std::path::Path::new(&dir).join("config.json"))
}

fn non_empty(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.is_empty())
}

fn load() -> Config {
    let mut cfg = Config::default();

    let file = config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    if let Some(v) = file.as_ref() {
        if let Some(list) = v.get("statuses").and_then(|s| s.as_array()) {
            let statuses: Vec<String> = list
                .iter()
                .filter_map(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            // An empty or all-junk list is not "a board with no statuses",
            // which would render nothing at all; it is a config that failed to
            // say anything, and the default is the better answer.
            if !statuses.is_empty() {
                cfg.statuses = statuses;
            }
        }
        if let Some(b) = v.get("show_closed").and_then(|s| s.as_bool()) {
            cfg.show_closed = b;
        }
    }

    if let Some(list) = non_empty("HERDR_DRIP_BEADS_STATUSES") {
        let statuses: Vec<String> = list
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        if !statuses.is_empty() {
            cfg.statuses = statuses;
        }
    }
    if let Some(flag) = non_empty("HERDR_DRIP_BEADS_SHOW_CLOSED") {
        cfg.show_closed = matches!(flag.to_ascii_lowercase().as_str(), "1" | "true" | "yes");
    }

    cfg
}
