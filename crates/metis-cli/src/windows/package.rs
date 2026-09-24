//! Application inventory materialization into one transactional MSI package.
use super::database::{
    Database,
    Value::{Null, Number, Stream, Text},
    guid,
};
use super::{payload, schema, shortcut, url_schemes};
use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
};

// This package rejects ALLUSERS, so its inventory is always per-user. Use the
// explicit MSI current-user root rather than the context-dependent -1 root;
// the latter is resolved by the elevated Windows Installer service and does
// not produce values in the installing user's hive on the supported host.
const USER_REGISTRY_ROOT: i32 = 1;

pub(crate) struct InstallerSpec<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub manufacturer: &'a str,
    pub upgrade_code: &'a str,
    pub entry: &'a str,
    pub arguments: &'a [String],
    pub url_schemes: &'a [String],
    pub files: &'a [(PathBuf, String)],
    pub icon: Option<&'a Path>,
}

/// Reads package identity and cabinet membership through the native MSI reader.
/// This verifies package structure; only installation verifies standard actions.
pub(crate) fn inspect(path: &Path) -> Result<(String, Vec<String>), Box<dyn Error>> {
    let database = Database::open(path)?;
    let products =
        database.strings("SELECT `Value` FROM `Property` WHERE `Property`='ProductCode'")?;
    let [product] = products.as_slice() else {
        return Err("MSI must contain exactly one ProductCode".into());
    };

    let files = database.strings("SELECT `FileName` FROM `File` ORDER BY `Sequence`")?;
    Ok((product.clone(), files))
}

pub(crate) fn build(spec: &InstallerSpec<'_>, output: &Path) -> Result<String, Box<dyn Error>> {
    payload::validate(spec, output)?;
    shortcut::arguments(spec.arguments)?;
    let product = guid()?;
    let package = guid()?;
    let parent = output.parent().ok_or("MSI output has no parent")?;
    let staging = parent.join(format!(".metis-msi-{}", &package[1..37]));
    std::fs::create_dir(&staging)?;
    let result = (|| {
        let cabinet = payload::cabinet(spec, &staging)?;
        let temporary = staging.join("package.msi");
        {
            let database = Database::create(&temporary)?;
            database.codepage(&staging)?;
            schema::create(&database)?;
            populate(&database, spec, &product, &cabinet)?;
            database.summary(&package, spec.manufacturer)?;
            database.commit()?;
        }
        // Hard-link publication cannot replace an existing output, unlike rename
        // on some platforms; both paths are on the same filesystem.
        std::fs::hard_link(&temporary, output)?;
        Ok(product)
    })();
    // Delete only this invocation's fixed filenames. Never recurse into a caller
    // supplied tree, and retain unknown contents if another actor added any.
    let cleanup = payload::cleanup(&staging);
    match (result, cleanup) {
        (Ok(product), Ok(())) => Ok(product),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(format!("{error}; staging cleanup failed: {cleanup}").into())
        }
    }
}

fn populate(
    database: &Database,
    spec: &InstallerSpec<'_>,
    product: &str,
    cabinet: &Path,
) -> Result<(), Box<dyn Error>> {
    for (key, value) in [
        ("ProductCode", product),
        ("UpgradeCode", spec.upgrade_code),
        ("ProductName", spec.name),
        ("ProductVersion", spec.version),
        ("Manufacturer", spec.manufacturer),
        ("ProductLanguage", "1033"),
        ("INSTALLLEVEL", "1"),
        ("ARPNOMODIFY", "1"),
        ("SecureCustomProperties", "INSTALLDIR;METISRELATED"),
    ] {
        schema::property(database, key, value)?;
    }
    // ALLUSERS is intentionally absent: this package has exactly one context,
    // the installing user. The launch condition rejects command-line escalation.
    for (condition, message) in [
        (
            "NOT ALLUSERS",
            "This package installs for the current user only.",
        ),
        (
            "Installed OR NOT METISRELATED",
            "A related application is installed. Uninstall it before installing this package.",
        ),
        ("VersionNT64", "This package requires 64-bit Windows."),
        (
            "NOT Installed OR INSTALLDIR",
            "The installed application location is missing. Restore its InstallLocation registry value before maintenance.",
        ),
    ] {
        database.execute(
            "INSERT INTO `LaunchCondition` (`Condition`,`Description`) VALUES (?,?)",
            &[Text(condition), Text(message)],
        )?;
    }
    // OnlyDetect | VersionMinInclusive, no maximum: older, equal, and newer
    // related products all reject. Same ProductCode maintenance remains allowed.
    database.execute("INSERT INTO `Upgrade` (`UpgradeCode`,`VersionMin`,`VersionMax`,`Language`,`Attributes`,`Remove`,`ActionProperty`) VALUES (?,?,?,?,?,?,?)", &[Text(spec.upgrade_code), Text("0.0.0"), Null, Null, Number(258), Null, Text("METISRELATED")])?;
    write_icon(database, spec)?;
    let registry_key = format!("Software\\Metis\\Applications\\{}", spec.id);
    // A registry keypath cannot recover the component's file directory. Search
    // the recorded REG_SZ before costing, including when files are missing.
    // RawValue | 64bit matches the entry component's registry view; the empty
    // Signature table distinguishes this lookup from a file signature search.
    // https://learn.microsoft.com/en-us/windows/win32/msi/reglocator-table
    database.execute(
        "INSERT INTO `RegLocator` (`Signature_`,`Root`,`Key`,`Name`,`Type`) VALUES (?,?,?,?,?)",
        &[
            Text("InstallLocation"),
            Number(1),
            Text(&registry_key),
            Text("InstallLocation"),
            Number(18),
        ],
    )?;
    database.execute(
        "INSERT INTO `AppSearch` (`Property`,`Signature_`) VALUES (?,?)",
        &[Text("INSTALLDIR"), Text("InstallLocation")],
    )?;
    schema::directory(database, "TARGETDIR", None, "SourceDir")?;
    schema::directory(database, "LocalAppDataFolder", Some("TARGETDIR"), ".")?;
    schema::directory(
        database,
        "INSTALLDIR",
        Some("LocalAppDataFolder"),
        &format!("{}|{}", &product[1..9], spec.id),
    )?;
    schema::directory(database, "ProgramMenuFolder", Some("TARGETDIR"), ".")?;
    schema::directory(
        database,
        "APPLICATIONMENU",
        Some("ProgramMenuFolder"),
        &format!("{}|{}", &product[1..9], spec.id),
    )?;
    database.execute("INSERT INTO `Feature` (`Feature`,`Feature_Parent`,`Title`,`Description`,`Display`,`Level`,`Directory_`,`Attributes`) VALUES (?,?,?,?,?,?,?,?)", &[Text("Application"), Null, Text("Application"), Null, Number(1), Number(1), Text("INSTALLDIR"), Number(0)])?;
    let mut directories = BTreeMap::from([(String::new(), "INSTALLDIR".to_owned())]);
    for (index, (source, destination)) in spec.files.iter().enumerate() {
        let (parent, name) = destination
            .rsplit_once('/')
            .unwrap_or(("", destination.as_str()));
        let directory = make_directories(database, &mut directories, parent)?;
        let file = format!("F{index}");
        let component = format!("C{index}");
        let registry = format!("R{index}");
        let component_guid = component_guid(spec.id, destination);
        // The file is the component key path. Registry values remain owned by
        // the component, but using them as key paths makes every rebuild with
        // a different component GUID collide with stale Windows Installer
        // component records from an older package.
        database.execute("INSERT INTO `Component` (`Component`,`ComponentId`,`Directory_`,`Attributes`,`Condition`,`KeyPath`) VALUES (?,?,?,?,?,?)", &[Text(&component), Text(&component_guid), Text(&directory), Number(256), Null, Text(&file)])?;
        database.execute(
            "INSERT INTO `FeatureComponents` (`Feature_`,`Component_`) VALUES (?,?)",
            &[Text("Application"), Text(&component)],
        )?;
        database.execute("INSERT INTO `File` (`File`,`Component_`,`FileName`,`FileSize`,`Version`,`Language`,`Attributes`,`Sequence`) VALUES (?,?,?,?,?,?,?,?)", &[Text(&file), Text(&component), Text(&format!("F{index}|{name}")), Number(i32::try_from(source.metadata()?.len())?), Null, Null, Number(16384), Number(i32::try_from(index + 1)?)])?;
        database.execute("INSERT INTO `Registry` (`Registry`,`Root`,`Key`,`Name`,`Value`,`Component_`) VALUES (?,?,?,?,?,?)", &[Text(&registry), Number(USER_REGISTRY_ROOT), Text(&registry_key), Text(&file), Text(spec.version), Text(&component)])?;
        if destination == spec.entry {
            write_entry(database, spec, product, &registry_key, &component, &file)?;
        }
    }
    database.execute("INSERT INTO `Media` (`DiskId`,`LastSequence`,`DiskPrompt`,`Cabinet`,`VolumeLabel`,`Source`) VALUES (?,?,?,?,?,?)", &[Number(1), Number(i32::try_from(spec.files.len())?), Null, Text("#payload.cab"), Null, Null])?;
    database.execute(
        "INSERT INTO `_Streams` (`Name`,`Data`) VALUES (?,?)",
        &[Text("payload.cab"), Stream(cabinet)],
    )
}

pub(super) fn component_guid(application_id: &str, destination: &str) -> String {
    // MSI component identity is stable for one application resource. A fresh
    // GUID on every package rebuild registers the same registry key path as a
    // different component and leaves repeated per-user installs without their
    // registry values. UUID version 5 gives the path a deterministic identity.
    let mut name = Vec::with_capacity(application_id.len() + destination.len() + 24);
    name.extend_from_slice(b"metis-msi-component\0");
    name.extend_from_slice(application_id.as_bytes());
    name.push(0);
    name.extend_from_slice(destination.as_bytes());
    let digest = moirai_crypto::sha1(&name);
    let mut bytes = [0_u8; 16];
    let length = bytes.len();
    bytes.copy_from_slice(&digest[..length]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{{{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn write_icon(database: &Database, spec: &InstallerSpec<'_>) -> Result<(), Box<dyn Error>> {
    if let Some(icon) = spec.icon {
        database.execute(
            "INSERT INTO `Icon` (`Name`,`Data`) VALUES (?,?)",
            &[Text("MetisIcon"), Stream(icon)],
        )?;
    }
    Ok(())
}

/// The entry component's extra rows: the maintenance location, the menu
/// folder and shortcut, and its URL-scheme registration.
fn write_entry(
    database: &Database,
    spec: &InstallerSpec<'_>,
    product: &str,
    registry_key: &str,
    component: &str,
    file: &str,
) -> Result<(), Box<dyn Error>> {
    let arguments = shortcut::arguments(spec.arguments)?;
    database.execute("INSERT INTO `Registry` (`Registry`,`Root`,`Key`,`Name`,`Value`,`Component_`) VALUES (?,?,?,?,?,?)", &[Text("InstallLocation"), Number(USER_REGISTRY_ROOT), Text(registry_key), Text("InstallLocation"), Text("[INSTALLDIR]"), Text(component)])?;
    database.execute(
        "INSERT INTO `CreateFolder` (`Directory_`,`Component_`) VALUES (?,?)",
        &[Text("APPLICATIONMENU"), Text(component)],
    )?;
    write_shortcut(database, spec, product, component, file, &arguments)?;
    url_schemes::register(
        database,
        USER_REGISTRY_ROOT,
        spec.url_schemes,
        spec.name,
        component,
        file,
    )
}

fn write_shortcut(
    database: &Database,
    spec: &InstallerSpec<'_>,
    product: &str,
    component: &str,
    file: &str,
    arguments: &str,
) -> Result<(), Box<dyn Error>> {
    let (icon_name, icon_index) = if spec.icon.is_some() {
        (Text("MetisIcon"), Number(0))
    } else {
        (Null, Null)
    };
    database.execute("INSERT INTO `Shortcut` (`Shortcut`,`Directory_`,`Name`,`Component_`,`Target`,`Arguments`,`Description`,`Hotkey`,`Icon_`,`IconIndex`,`ShowCmd`,`WkDir`) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)", &[Text("ApplicationShortcut"), Text("APPLICATIONMENU"), Text(&format!("{}|{}.lnk", &product[1..9], spec.name)), Text(component), Text(&format!("[#{file}]")), Text(arguments), Text(spec.name), Null, icon_name, icon_index, Number(1), Text("INSTALLDIR")])
}

fn make_directories(
    database: &Database,
    directories: &mut BTreeMap<String, String>,
    path: &str,
) -> Result<String, Box<dyn Error>> {
    let mut parent = "INSTALLDIR".to_owned();
    let mut partial = String::new();
    for name in path.split('/').filter(|part| !part.is_empty()) {
        if !partial.is_empty() {
            partial.push('/');
        }
        partial.push_str(name);
        if let Some(existing) = directories.get(&partial) {
            parent.clone_from(existing);
        } else {
            let directory = format!("D{}", directories.len());
            schema::directory(
                database,
                &directory,
                Some(&parent),
                &format!("{directory}|{name}"),
            )?;
            directories.insert(partial.clone(), directory.clone());
            parent = directory;
        }
    }
    Ok(parent)
}
