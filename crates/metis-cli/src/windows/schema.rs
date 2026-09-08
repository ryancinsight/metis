//! MSI relational schema and standard transactional action ordering.
//!
//! Field types follow Microsoft's [table reference](https://learn.microsoft.com/en-us/windows/win32/msi/database-tables).
//! No wildcard removal record or custom action is authored: removal is limited
//! to the File/Registry/Shortcut ownership records below.
use super::database::{
    Database,
    Value::{Null, Number, Text},
};
use std::error::Error;

pub(super) fn create(database: &Database) -> Result<(), Box<dyn Error>> {
    for sql in [
        "CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(0) NOT NULL LOCALIZABLE PRIMARY KEY `Property`)",
        "CREATE TABLE `AppSearch` (`Property` CHAR(72) NOT NULL, `Signature_` CHAR(72) NOT NULL PRIMARY KEY `Property`, `Signature_`)",
        "CREATE TABLE `RegLocator` (`Signature_` CHAR(72) NOT NULL, `Root` SHORT NOT NULL, `Key` CHAR(255) NOT NULL, `Name` CHAR(255), `Type` SHORT PRIMARY KEY `Signature_`)",
        "CREATE TABLE `Signature` (`Signature` CHAR(72) NOT NULL, `FileName` CHAR(255) NOT NULL, `MinVersion` CHAR(20), `MaxVersion` CHAR(20), `MinSize` LONG, `MaxSize` LONG, `MinDate` LONG, `MaxDate` LONG, `Languages` CHAR(255) PRIMARY KEY `Signature`)",
        "CREATE TABLE `Directory` (`Directory` CHAR(72) NOT NULL, `Directory_Parent` CHAR(72), `DefaultDir` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY `Directory`)",
        "CREATE TABLE `Component` (`Component` CHAR(72) NOT NULL, `ComponentId` CHAR(38), `Directory_` CHAR(72) NOT NULL, `Attributes` SHORT NOT NULL, `Condition` CHAR(255), `KeyPath` CHAR(72) PRIMARY KEY `Component`)",
        "CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL, `Feature_Parent` CHAR(38), `Title` CHAR(64) LOCALIZABLE, `Description` CHAR(255) LOCALIZABLE, `Display` SHORT, `Level` SHORT NOT NULL, `Directory_` CHAR(72), `Attributes` SHORT NOT NULL PRIMARY KEY `Feature`)",
        "CREATE TABLE `FeatureComponents` (`Feature_` CHAR(38) NOT NULL, `Component_` CHAR(72) NOT NULL PRIMARY KEY `Feature_`, `Component_`)",
        "CREATE TABLE `CreateFolder` (`Directory_` CHAR(72) NOT NULL, `Component_` CHAR(72) NOT NULL PRIMARY KEY `Directory_`, `Component_`)",
        "CREATE TABLE `File` (`File` CHAR(72) NOT NULL, `Component_` CHAR(72) NOT NULL, `FileName` CHAR(255) NOT NULL LOCALIZABLE, `FileSize` LONG NOT NULL, `Version` CHAR(72), `Language` CHAR(20), `Attributes` SHORT, `Sequence` SHORT NOT NULL PRIMARY KEY `File`)",
        "CREATE TABLE `Media` (`DiskId` SHORT NOT NULL, `LastSequence` SHORT NOT NULL, `DiskPrompt` CHAR(64) LOCALIZABLE, `Cabinet` CHAR(255), `VolumeLabel` CHAR(32), `Source` CHAR(72) PRIMARY KEY `DiskId`)",
        "CREATE TABLE `Registry` (`Registry` CHAR(72) NOT NULL, `Root` SHORT NOT NULL, `Key` CHAR(255) NOT NULL LOCALIZABLE, `Name` CHAR(255) LOCALIZABLE, `Value` CHAR(0) LOCALIZABLE, `Component_` CHAR(72) NOT NULL PRIMARY KEY `Registry`)",
        "CREATE TABLE `Icon` (`Name` CHAR(72) NOT NULL, `Data` OBJECT NOT NULL PRIMARY KEY `Name`)",
        "CREATE TABLE `Shortcut` (`Shortcut` CHAR(72) NOT NULL, `Directory_` CHAR(72) NOT NULL, `Name` CHAR(128) NOT NULL LOCALIZABLE, `Component_` CHAR(72) NOT NULL, `Target` CHAR(72) NOT NULL, `Arguments` CHAR(255), `Description` CHAR(255) LOCALIZABLE, `Hotkey` SHORT, `Icon_` CHAR(72), `IconIndex` SHORT, `ShowCmd` SHORT, `WkDir` CHAR(72) PRIMARY KEY `Shortcut`)",
        "CREATE TABLE `Upgrade` (`UpgradeCode` CHAR(38) NOT NULL, `VersionMin` CHAR(20), `VersionMax` CHAR(20), `Language` CHAR(255), `Attributes` SHORT NOT NULL, `Remove` CHAR(255), `ActionProperty` CHAR(72) NOT NULL PRIMARY KEY `UpgradeCode`, `VersionMin`, `VersionMax`, `Language`, `Attributes`)",
        "CREATE TABLE `LaunchCondition` (`Condition` CHAR(255) NOT NULL, `Description` CHAR(255) NOT NULL LOCALIZABLE PRIMARY KEY `Condition`)",
        "CREATE TABLE `InstallExecuteSequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)",
        "CREATE TABLE `InstallUISequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)",
    ] {
        database.execute(sql, &[])?;
    }
    for (action, sequence) in [
        ("FindRelatedProducts", 25),
        ("AppSearch", 50),
        ("LaunchConditions", 100),
        ("ValidateProductID", 700),
        ("CostInitialize", 800),
        ("FileCost", 900),
        ("CostFinalize", 1000),
        ("InstallValidate", 1400),
        ("InstallInitialize", 1500),
        ("ProcessComponents", 1600),
        ("UnpublishFeatures", 1800),
        ("RemoveShortcuts", 3200),
        ("RemoveRegistryValues", 2600),
        ("RemoveFiles", 3500),
        ("RemoveFolders", 3600),
        ("CreateFolders", 3700),
        ("InstallFiles", 4000),
        ("CreateShortcuts", 4500),
        ("WriteRegistryValues", 5000),
        ("RegisterUser", 6000),
        ("RegisterProduct", 6100),
        ("PublishFeatures", 6300),
        ("PublishProduct", 6400),
        ("InstallFinalize", 6600),
    ] {
        let condition = if action == "AppSearch" {
            Text("Installed")
        } else {
            Null
        };
        let values = [Text(action), condition, Number(sequence)];
        database.execute(
            "INSERT INTO `InstallExecuteSequence` (`Action`,`Condition`,`Sequence`) VALUES (?,?,?)",
            &values,
        )?;
        if sequence < 1400 {
            database.execute(
                "INSERT INTO `InstallUISequence` (`Action`,`Condition`,`Sequence`) VALUES (?,?,?)",
                &values,
            )?;
        }
    }
    database.execute(
        "INSERT INTO `InstallUISequence` (`Action`,`Condition`,`Sequence`) VALUES (?,?,?)",
        &[Text("ExecuteAction"), Null, Number(1300)],
    )?;
    Ok(())
}

pub(super) fn property(database: &Database, key: &str, value: &str) -> Result<(), Box<dyn Error>> {
    database.execute(
        "INSERT INTO `Property` (`Property`,`Value`) VALUES (?,?)",
        &[Text(key), Text(value)],
    )
}

pub(super) fn directory(
    database: &Database,
    key: &str,
    parent: Option<&str>,
    name: &str,
) -> Result<(), Box<dyn Error>> {
    database.execute(
        "INSERT INTO `Directory` (`Directory`,`Directory_Parent`,`DefaultDir`) VALUES (?,?,?)",
        &[Text(key), parent.map_or(Null, Text), Text(name)],
    )
}
