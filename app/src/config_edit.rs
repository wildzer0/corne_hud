//! Changes to `keyboard-hud.toml` made by the app (settings window, menu):
//! only the keys touched are rewritten, comments and layout stay as they are.

use crate::config::Config;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

/// A dotted key (`appearance.scale`) and its new value; `None` removes the
/// key, so that the default applies.
pub type Edit = (&'static str, Option<toml::Value>);

/// Written at the top of a config file created by the app.
const HEADER: &str = "# Keyboard HUD: configurazione. Modificabile a mano o dalla finestra\n\
# Impostazioni; i commenti restano. Le voci mancanti hanno il valore predefinito.\n";

/// Floats as people write them: 0.85, not 0.8500000238418579 (from f32).
fn to_edit(v: &toml::Value) -> Result<Value, String> {
    Ok(match v {
        toml::Value::String(s) => Value::from(s.as_str()),
        toml::Value::Integer(i) => Value::from(*i),
        toml::Value::Float(f) => Value::from((f * 10_000.0).round() / 10_000.0),
        toml::Value::Boolean(b) => Value::from(*b),
        other => return Err(format!("valore non supportato: {other}")),
    })
}

/// Applies the edits to the text of a config file. A file that does not
/// parse is left alone: the user is in the middle of editing it.
pub fn apply(text: &str, edits: &[Edit]) -> Result<String, String> {
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    // comments only (a new file, its header): they belong above the first table,
    // not after the tables added
    let header = doc.is_empty().then(|| doc.trailing().as_str().unwrap_or("").to_string());
    for (key, value) in edits {
        let (path, leaf) = match key.rsplit_once('.') {
            Some((p, l)) => (p.split('.').collect::<Vec<_>>(), l),
            None => (Vec::new(), *key),
        };
        let mut table: &mut dyn TableLike = doc.as_table_mut();
        for part in path {
            if value.is_none() && table.get(part).is_none() {
                break; // nothing to remove
            }
            let item = table.entry(part).or_insert_with(|| Item::Table(Table::new()));
            table = item.as_table_like_mut().ok_or_else(|| format!("{key}: {part} non è una tabella"))?;
        }
        match value {
            Some(v) => set(table, leaf, to_edit(v)?),
            None => remove(table, leaf),
        }
    }
    if let Some(header) = header.filter(|h| !h.trim().is_empty()) {
        let moved = match doc.as_table_mut().iter_mut().next() {
            Some((_, Item::Table(first))) => {
                first.decor_mut().set_prefix(header);
                true
            }
            _ => false,
        };
        if moved {
            doc.set_trailing("");
        }
    }
    Ok(doc.to_string())
}

/// Replaces the value keeping its comment (`scale = 1.2   # 0.5 – 2.0`).
fn set(table: &mut dyn TableLike, key: &str, mut value: Value) {
    // in place: inserting again would lose the comment lines above the key
    if let Some(old) = table.get_mut(key).and_then(Item::as_value_mut) {
        *value.decor_mut() = old.decor().clone();
        *old = value;
    } else {
        value.decor_mut().set_prefix(" ");
        table.insert(key, Item::Value(value));
    }
}

/// Removes the key; the comment lines above it move to the next key.
fn remove(table: &mut dyn TableLike, key: &str) {
    let prefix = table.key(key).and_then(|k| k.leaf_decor().prefix()).and_then(|p| p.as_str()).map(str::to_string);
    let next = table.iter().map(|(k, _)| k.to_string()).skip_while(|k| k != key).nth(1);
    table.remove(key);
    if let (Some(prefix), Some(next)) = (prefix.filter(|p| p.contains('#')), next) {
        if let Some(mut k) = table.key_mut(&next) {
            let own = k.leaf_decor().prefix().and_then(|p| p.as_str()).unwrap_or("").to_string();
            k.leaf_decor_mut().set_prefix(format!("{prefix}{}", own.trim_start_matches(' ')));
        }
    }
}

/// Edits the config file on disk (created if missing) and returns the
/// resulting configuration. Nothing is written if the result is not valid.
pub fn write(path: &Path, edits: &[Edit]) -> Result<Config, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HEADER.to_string(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let new = apply(&text, edits)?;
    let config = Config::parse(&new)?;
    // through a temporary file: an editor or the watcher never sees half a file
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, &new)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float(f: f32) -> Option<toml::Value> {
        Some(toml::Value::Float(f as f64))
    }

    #[test]
    fn keeps_comments_and_layout() {
        let text = "# intestazione\n[appearance]\n# commento sopra\nscale = 1.0   # 0.5 – 2.0\ntheme = \"dark\"\n\n[log]\nlevel = \"info\"\n";
        let out = apply(text, &[("appearance.scale", float(1.25))]).unwrap();
        assert_eq!(out, text.replace("scale = 1.0", "scale = 1.25"));
    }

    #[test]
    fn rounds_floats_from_f32() {
        let out = apply("", &[("appearance.opacity", float(0.85))]).unwrap();
        assert_eq!(out, "[appearance]\nopacity = 0.85\n");
    }

    #[test]
    fn adds_tables_and_keys() {
        let text = "[appearance]\nscale = 1.0\n";
        let edits = [
            ("appearance.theme", Some(toml::Value::String("light".into()))),
            ("keycast.enabled", Some(toml::Value::Boolean(true))),
        ];
        let out = apply(text, &edits).unwrap();
        assert_eq!(out, "[appearance]\nscale = 1.0\ntheme = \"light\"\n\n[keycast]\nenabled = true\n");
        assert!(Config::parse(&out).unwrap().keycast.enabled);
    }

    #[test]
    fn removal_keeps_the_comment_above() {
        let text = "[appearance]\n# chiave\nkey_opacity = 0.5\n# stile\nstyle = \"vetro\"\n";
        let out = apply(text, &[("appearance.key_opacity", None), ("device.vid", None)]).unwrap();
        assert_eq!(out, "[appearance]\n# chiave\n# stile\nstyle = \"vetro\"\n");
    }

    #[test]
    fn header_of_a_new_file_stays_on_top() {
        let out = apply(
            HEADER,
            &[("appearance.scale", float(1.5)), ("log.level", Some(toml::Value::String("debug".into())))],
        )
        .unwrap();
        assert_eq!(out, format!("{HEADER}[appearance]\nscale = 1.5\n\n[log]\nlevel = \"debug\"\n"));
        assert_eq!(apply(HEADER, &[]).unwrap(), HEADER);
    }

    #[test]
    fn invalid_file_is_left_alone() {
        assert!(apply("[appearance\nscale = 1", &[("appearance.scale", float(1.0))]).is_err());
        assert!(apply("appearance = 3", &[("appearance.scale", float(1.0))]).is_err());
    }

    #[test]
    fn inline_tables_too() {
        let out = apply("appearance = { scale = 1.0 }\n", &[("appearance.scale", float(1.5))]).unwrap();
        assert_eq!(out, "appearance = { scale = 1.5 }\n");
    }

    #[test]
    fn example_config_survives() {
        let example = include_str!("../../keyboard-hud.example.toml");
        let out = apply(example, &[("appearance.scale", float(1.2))]).unwrap();
        assert_eq!(out, example.replace("scale = 1.0", "scale = 1.2"));
    }
}
