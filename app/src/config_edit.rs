//! Changes to `keyboard-hud.toml` made by the app (settings window, menu):
//! only the keys touched are rewritten, comments and layout stay as they are.

use crate::config::Config;
use std::path::Path;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, TableLike, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// A dotted key (`appearance.scale`) and its new value; `None` removes
    /// the key, so that the default applies.
    Key(&'static str, Option<toml::Value>),
    /// A field of one element of a list of tables (`[[layers]]`); `None` removes it.
    Field { list: &'static str, index: usize, field: &'static str, value: Option<toml::Value> },
    /// A new element at the end of a list of tables.
    Push { list: &'static str, fields: Vec<(&'static str, toml::Value)> },
    /// Removes an element of a list of tables.
    Remove { list: &'static str, index: usize },
}

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
        toml::Value::Array(a) => Value::Array(a.iter().map(to_edit).collect::<Result<_, _>>()?),
        other => return Err(format!("unsupported value: {other}")),
    })
}

/// Applies the edits to the text of a config file. A file that does not
/// parse is left alone: the user is in the middle of editing it.
pub fn apply(text: &str, edits: &[Edit]) -> Result<String, String> {
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    // comments only (a new file, its header): they belong above the first table,
    // not after the tables added
    let header = doc.is_empty().then(|| doc.trailing().as_str().unwrap_or("").to_string());
    for edit in edits {
        match edit {
            Edit::Key(key, value) => set_key(&mut doc, key, value.as_ref())?,
            Edit::Field { list, index, field, value } => {
                let table = list_mut(&mut doc, list)?
                    .get_mut(*index)
                    .ok_or_else(|| format!("{list}: missing item {}", index + 1))?;
                match value {
                    Some(v) => set(table, field, to_edit(v)?),
                    None => remove(table, field),
                }
            }
            Edit::Push { list, fields } => {
                let mut table = Table::new();
                for (field, v) in fields {
                    table.insert(field, Item::Value(to_edit(v)?));
                }
                if doc.get(list).is_none() {
                    doc.insert(list, Item::ArrayOfTables(ArrayOfTables::new()));
                }
                list_mut(&mut doc, list)?.push(table);
            }
            Edit::Remove { list, index } => remove_element(&mut doc, list, *index)?,
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
    Ok(with_line_endings_of(text, doc.to_string()))
}

/// `toml_edit` writes `\n`: a file with `\r\n` (Notepad) keeps them.
pub fn with_line_endings_of(original: &str, text: String) -> String {
    if original.contains("\r\n") {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text
    }
}

fn set_key(doc: &mut DocumentMut, key: &str, value: Option<&toml::Value>) -> Result<(), String> {
    let (path, leaf) = match key.rsplit_once('.') {
        Some((p, l)) => (p.split('.').collect::<Vec<_>>(), l),
        None => (Vec::new(), key),
    };
    let mut table: &mut dyn TableLike = doc.as_table_mut();
    for part in path {
        if value.is_none() && table.get(part).is_none() {
            return Ok(()); // nothing to remove
        }
        let item = table.entry(part).or_insert_with(|| Item::Table(Table::new()));
        table = item.as_table_like_mut().ok_or_else(|| format!("{key}: {part} is not a table"))?;
    }
    match value {
        Some(v) => set(table, leaf, to_edit(v)?),
        None => remove(table, leaf),
    }
    Ok(())
}

fn list_mut<'a>(doc: &'a mut DocumentMut, list: &str) -> Result<&'a mut ArrayOfTables, String> {
    doc.get_mut(list)
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| format!("{list}: not an array of tables [[{list}]]"))
}

/// Removes an element; the comment lines above it go to whatever comes next
/// (the next element, or the next table in the file).
fn remove_element(doc: &mut DocumentMut, list: &str, index: usize) -> Result<(), String> {
    let tables = list_mut(doc, list)?;
    let old = tables.get(index).ok_or_else(|| format!("{list}: missing item {}", index + 1))?;
    let prefix = old.decor().prefix().and_then(|p| p.as_str()).unwrap_or("").to_string();
    let position = old.position();
    tables.remove(index);
    if tables.is_empty() {
        doc.remove(list);
    }
    if !prefix.contains('#') {
        return Ok(());
    }
    let next = match list_mut(doc, list).ok().and_then(|t| t.get_mut(index)) {
        Some(t) => Some(t),
        None => next_table(doc, position),
    };
    match next {
        Some(t) => {
            let own = t.decor().prefix().and_then(|p| p.as_str()).unwrap_or("").to_string();
            t.decor_mut().set_prefix(format!("{prefix}{}", own.trim_start_matches('\n')));
        }
        None => {
            let trailing = doc.trailing().as_str().unwrap_or("").to_string();
            doc.set_trailing(format!("{prefix}{trailing}"));
        }
    }
    Ok(())
}

/// The table written first after `position`, at the top level.
fn next_table(doc: &mut DocumentMut, position: Option<isize>) -> Option<&mut Table> {
    let after = position?;
    let mut tables: Vec<&mut Table> = Vec::new();
    for (_, item) in doc.as_table_mut().iter_mut() {
        match item {
            Item::Table(t) => tables.push(t),
            Item::ArrayOfTables(a) => tables.extend(a.iter_mut()),
            _ => {}
        }
    }
    tables.into_iter().filter(|t| t.position().is_some_and(|p| p > after)).min_by_key(|t| t.position())
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
    write_text(path, &apply(&text, edits)?)
}

/// Replaces the whole file (text edited in the settings window), if valid.
pub fn write_text(path: &Path, text: &str) -> Result<Config, String> {
    let config = Config::parse(text)?;
    // through a temporary file: an editor or the watcher never sees half a file
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)
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

    fn key(k: &'static str, v: Option<toml::Value>) -> Edit {
        Edit::Key(k, v)
    }

    #[test]
    fn keeps_comments_and_layout() {
        let text = "# intestazione\n[appearance]\n# commento sopra\nscale = 1.0   # 0.5 – 2.0\ntheme = \"dark\"\n\n[log]\nlevel = \"info\"\n";
        let out = apply(text, &[key("appearance.scale", float(1.25))]).unwrap();
        assert_eq!(out, text.replace("scale = 1.0", "scale = 1.25"));
    }

    #[test]
    fn rounds_floats_from_f32() {
        let out = apply("", &[key("appearance.opacity", float(0.85))]).unwrap();
        assert_eq!(out, "[appearance]\nopacity = 0.85\n");
    }

    #[test]
    fn adds_tables_and_keys() {
        let text = "[appearance]\nscale = 1.0\n";
        let edits = [
            key("appearance.theme", Some(toml::Value::String("light".into()))),
            key("keycast.enabled", Some(toml::Value::Boolean(true))),
        ];
        let out = apply(text, &edits).unwrap();
        assert_eq!(out, "[appearance]\nscale = 1.0\ntheme = \"light\"\n\n[keycast]\nenabled = true\n");
        assert!(Config::parse(&out).unwrap().keycast.enabled);
    }

    #[test]
    fn removal_keeps_the_comment_above() {
        let text = "[appearance]\n# chiave\nkey_opacity = 0.5\n# stile\nstyle = \"vetro\"\n";
        let out = apply(text, &[key("appearance.key_opacity", None), key("device.vid", None)]).unwrap();
        assert_eq!(out, "[appearance]\n# chiave\n# stile\nstyle = \"vetro\"\n");
    }

    #[test]
    fn header_of_a_new_file_stays_on_top() {
        let out = apply(
            HEADER,
            &[key("appearance.scale", float(1.5)), key("log.level", Some(toml::Value::String("debug".into())))],
        )
        .unwrap();
        assert_eq!(out, format!("{HEADER}[appearance]\nscale = 1.5\n\n[log]\nlevel = \"debug\"\n"));
        assert_eq!(apply(HEADER, &[]).unwrap(), HEADER);
    }

    #[test]
    fn invalid_file_is_left_alone() {
        assert!(apply("[appearance\nscale = 1", &[key("appearance.scale", float(1.0))]).is_err());
        assert!(apply("appearance = 3", &[key("appearance.scale", float(1.0))]).is_err());
    }

    #[test]
    fn inline_tables_too() {
        let out = apply("appearance = { scale = 1.0 }\n", &[key("appearance.scale", float(1.5))]).unwrap();
        assert_eq!(out, "appearance = { scale = 1.5 }\n");
    }

    #[test]
    fn windows_line_endings_stay() {
        let text = "# intestazione\r\n[appearance]\r\nscale = 1.0   # x\r\n";
        let out = apply(
            text,
            &[key("appearance.scale", float(1.5)), key("log.level", Some(toml::Value::String("debug".into())))],
        )
        .unwrap();
        assert_eq!(out, "# intestazione\r\n[appearance]\r\nscale = 1.5   # x\r\n\r\n[log]\r\nlevel = \"debug\"\r\n");
        let crlf = include_str!("../../keyboard-hud.example.toml").replace("\r\n", "\n").replace('\n', "\r\n");
        let out = apply(&crlf, &[key("appearance.scale", float(1.2))]).unwrap();
        assert_eq!(out, crlf.replace("scale = 1.0", "scale = 1.2"));
    }

    #[test]
    fn example_config_survives() {
        let example = include_str!("../../keyboard-hud.example.toml");
        let out = apply(example, &[key("appearance.scale", float(1.2))]).unwrap();
        assert_eq!(out, example.replace("scale = 1.0", "scale = 1.2"));
    }

    fn string(v: &str) -> toml::Value {
        toml::Value::String(v.into())
    }

    const LISTS: &str = "[appearance]\nscale = 1.0\n\n# Layer\n[[layers]]\nname = \"BASE\"\ncolor = \"\"\n\n[[layers]]\nname = \"SYM\"   # simboli\ncolor = \"#00B9D7\"\n\n# Etichette del layer DEBUG\n[[labels]]\nlayer = 4\nkeycode = \"KC_F2\"\nlabel = \"Rename\"\n\n# Esempio commentato\n# [[labels]]\n\n[polling]\nvisible_hz = 60\n";

    #[test]
    fn list_fields_keep_comments() {
        let edits = [
            Edit::Field { list: "layers", index: 1, field: "name", value: Some(string("SIMBOLI")) },
            Edit::Field { list: "layers", index: 0, field: "color", value: Some(string("#FF0000")) },
            Edit::Field { list: "labels", index: 0, field: "description", value: Some(string("Rinomina")) },
        ];
        let out = apply(LISTS, &edits).unwrap();
        let expected = LISTS
            .replace("name = \"SYM\"   # simboli", "name = \"SIMBOLI\"   # simboli")
            .replace("color = \"\"\n", "color = \"#FF0000\"\n")
            .replace("label = \"Rename\"\n", "label = \"Rename\"\ndescription = \"Rinomina\"\n");
        assert_eq!(out, expected);
        let bad = [Edit::Field { list: "layers", index: 5, field: "name", value: Some(string("X")) }];
        assert!(apply(LISTS, &bad).is_err());
    }

    #[test]
    fn push_goes_after_the_last_element() {
        let edits =
            [Edit::Push { list: "labels", fields: vec![("keycode", string("KC_F10")), ("label", string("Step"))] }];
        let out = apply(LISTS, &edits).unwrap();
        assert!(
            out.contains(
                "label = \"Rename\"\n\n[[labels]]\nkeycode = \"KC_F10\"\nlabel = \"Step\"\n\n# Esempio commentato"
            ),
            "{out}"
        );
        assert_eq!(Config::parse(&out).unwrap().labels.len(), 2);
        // a list that is not there yet
        let out = apply(
            "[log]\nlevel = \"info\"\n",
            &[Edit::Push { list: "layers", fields: vec![("name", string("BASE"))] }],
        )
        .unwrap();
        assert_eq!(Config::parse(&out).unwrap().layers[0].name, "BASE");
    }

    #[test]
    fn removal_moves_the_comments_on() {
        // the next element takes them
        let out = apply(LISTS, &[Edit::Remove { list: "layers", index: 0 }]).unwrap();
        assert!(out.contains("scale = 1.0\n\n# Layer\n[[layers]]\nname = \"SYM\""), "{out}");
        // the last one: the next table in the file
        let out = apply(LISTS, &[Edit::Remove { list: "labels", index: 0 }]).unwrap();
        assert!(!out.contains("[[labels]]\nlayer"), "{out}");
        assert!(out.contains("# Etichette del layer DEBUG\n# Esempio commentato\n# [[labels]]\n\n[polling]"), "{out}");
        assert!(Config::parse(&out).unwrap().labels.is_empty());
        assert!(apply(LISTS, &[Edit::Remove { list: "labels", index: 3 }]).is_err());
    }

    #[test]
    fn app_lists() {
        let apps = toml::Value::Array(vec![string("idea64"), string("pycharm64")]);
        let edits = [Edit::Field { list: "labels", index: 0, field: "app", value: Some(apps) }];
        let out = apply(LISTS, &edits).unwrap();
        assert!(out.contains("app = [\"idea64\", \"pycharm64\"]"), "{out}");
        Config::parse(&out).unwrap();
    }
}
