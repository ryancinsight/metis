//! Per-user document-type registration for the application entry.
//!
//! Each declared document type becomes a programmatic identifier
//! `HKCU\Software\Classes\<id>.document<n>` whose `open` verb starts the
//! installed entry with the document as its one argument, and each of its
//! extensions lists that identifier under `OpenWithProgids`. Listing adds the
//! application to "Open with" without taking over the user's chosen default.
//! The rows belong to the entry's component, so uninstall removes them.

use super::database::{
    Database,
    Value::{Null, Number, Text},
};
use crate::manifest::FileAssociation;
use std::error::Error;

/// Registers `associations` for the entry file `file` owned by `component`.
pub(super) fn register(
    database: &Database,
    root: i32,
    associations: &[FileAssociation],
    id: &str,
    component: &str,
    file: &str,
) -> Result<(), Box<dyn Error>> {
    let insert = "INSERT INTO `Registry` (`Registry`,`Root`,`Key`,`Name`,`Value`,`Component_`) VALUES (?,?,?,?,?,?)";
    let command = format!("\"[#{file}]\" \"%1\"");
    for (index, association) in associations.iter().enumerate() {
        let prog_id = format!("{id}.document{index}");
        let key = format!("Software\\Classes\\{prog_id}");
        let command_key = format!("{key}\\shell\\open\\command");
        let mut rows = vec![
            (
                format!("FD{index}"),
                key.clone(),
                None,
                Some(association.description.as_str()),
            ),
            (
                format!("FC{index}"),
                command_key,
                None,
                Some(command.as_str()),
            ),
        ];
        for (position, extension) in association.extensions.iter().enumerate() {
            // A named value without data lists the identifier.
            rows.push((
                format!("FE{index}_{position}"),
                format!("Software\\Classes\\.{extension}\\OpenWithProgids"),
                Some(prog_id.as_str()),
                None,
            ));
        }
        for (row, key, name, value) in &rows {
            database.execute(
                insert,
                &[
                    Text(row),
                    Number(root),
                    Text(key),
                    name.map_or(Null, Text),
                    value.map_or(Null, Text),
                    Text(component),
                ],
            )?;
        }
    }
    Ok(())
}
