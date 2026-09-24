//! Per-user URL-scheme registration for the application entry.
//!
//! Each declared scheme becomes `HKCU\Software\Classes\<scheme>` with the
//! `URL Protocol` marker and an `open` verb that starts the installed entry
//! with the link as its one argument. The rows belong to the entry's
//! component, so Windows Installer removes them with the application.

use super::database::{
    Database,
    Value::{Null, Number, Text},
};
use std::error::Error;

/// Registers `schemes` for the entry file `file` owned by `component`.
pub(super) fn register(
    database: &Database,
    root: i32,
    schemes: &[String],
    name: &str,
    component: &str,
    file: &str,
) -> Result<(), Box<dyn Error>> {
    let insert = "INSERT INTO `Registry` (`Registry`,`Root`,`Key`,`Name`,`Value`,`Component_`) VALUES (?,?,?,?,?,?)";
    let command = format!("\"[#{file}]\" \"%1\"");
    for (index, scheme) in schemes.iter().enumerate() {
        let key = format!("Software\\Classes\\{scheme}");
        let command_key = format!("{key}\\shell\\open\\command");
        let description = format!("URL:{name}");
        // A null name writes the key's default value; a null value under a
        // named entry writes the empty `URL Protocol` marker.
        for (suffix, key, value_name, value) in [
            ("D", key.as_str(), None, Some(description.as_str())),
            ("P", key.as_str(), Some("URL Protocol"), None),
            ("C", command_key.as_str(), None, Some(command.as_str())),
        ] {
            database.execute(
                insert,
                &[
                    Text(&format!("U{suffix}{index}")),
                    Number(root),
                    Text(key),
                    value_name.map_or(Null, Text),
                    value.map_or(Null, Text),
                    Text(component),
                ],
            )?;
        }
    }
    Ok(())
}
