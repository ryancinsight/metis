//! Real cabinet/database round-trip; installation is a separate OS workflow.
use super::{
    InstallerSpec, build,
    database::{Database, guid},
    inspect,
};

#[test]
fn authored_package_retains_payload_identity_and_actions() {
    let directory =
        std::env::temp_dir().join(format!("metis-msi-test-{}", guid().expect("OS GUID")));
    std::fs::create_dir(&directory).expect("isolated test directory");
    // std canonicalization adds the verbatim prefix rejected by makecab. Every
    // source, staging directory and /F argument must pass through native policy.
    let directory = directory.canonicalize().expect("canonical test directory");
    let source = directory.join("source.bin");
    std::fs::write(&source, b"input-sensitive cabinet content\n").expect("fixture content");
    let output = directory.join("test.msi");
    let files = [
        (source.clone(), "app.exe".to_owned()),
        (source.clone(), "assets/value.txt".to_owned()),
    ];
    let upgrade = guid().expect("upgrade code");
    let spec = InstallerSpec {
        id: "org.metis.package-test",
        name: "Metis Package Test",
        version: "1.2.3",
        manufacturer: "Métis O'Brien",
        upgrade_code: &upgrade,
        entry: "app.exe",
        arguments: &["60".into(), "2".into(), "0.2".into()],
        files: &files,
    };
    let product = build(&spec, &output).expect("real MSI build");
    let (actual_product, names) = inspect(&output).expect("native MSI inspection");
    assert_eq!(actual_product, product);
    assert_eq!(names, ["F0|app.exe", "F1|value.txt"]);
    {
        let database = Database::open(&output).expect("MSI open");
        assert_maintenance_location(&database);
        assert_eq!(
            shortcut_path(&database),
            "org.metis.package-test/Metis Package Test.lnk"
        );
        assert_eq!(
            database
                .strings("SELECT `Directory_` FROM `CreateFolder` WHERE `Component_`='C0'")
                .expect("owned menu directory"),
            ["APPLICATIONMENU"]
        );
        assert_eq!(
            database
                .strings("SELECT `Arguments` FROM `Shortcut`")
                .expect("shortcut arguments"),
            ["\"60\" \"2\" \"0.2\""]
        );
        assert_eq!(
            database
                .strings("SELECT `Value` FROM `Property` WHERE `Property`='Manufacturer'")
                .expect("manufacturer"),
            ["Métis O'Brien"]
        );
        assert_eq!(
            database
                .strings("SELECT `Name` FROM `_Streams` WHERE `Name`='payload.cab'")
                .expect("cabinet"),
            ["payload.cab"]
        );
        assert_eq!(
            database
                .strings(
                    "SELECT `Action` FROM `InstallExecuteSequence` WHERE `Action`='InstallFinalize'"
                )
                .expect("transaction action"),
            ["InstallFinalize"]
        );
        assert_eq!(
            database
                .strings("SELECT `Value` FROM `Property` WHERE `Property`='ALLUSERS'")
                .expect("installation context"),
            Vec::<String>::new()
        );
        assert_eq!(database.strings("SELECT `Condition` FROM `LaunchCondition` WHERE `Condition`='Installed OR NOT METISRELATED'").expect("upgrade guard"), ["Installed OR NOT METISRELATED"]);
    }
    // Distinct application identities may use the same display name. Their
    // actual authored directory/name pairs must never share a shortcut path.
    let second_output = directory.join("second.msi");
    let second_upgrade = guid().expect("second upgrade identity");
    build(
        &InstallerSpec {
            id: "org.metis.other-test",
            upgrade_code: &second_upgrade,
            ..spec
        },
        &second_output,
    )
    .expect("second application package");
    {
        let database = Database::open(&second_output).expect("second MSI open");
        assert_eq!(
            shortcut_path(&database),
            "org.metis.other-test/Metis Package Test.lnk"
        );
    }
    std::fs::remove_file(second_output).expect("remove second owned MSI");
    std::fs::remove_file(output).expect("remove owned MSI");
    std::fs::remove_file(source).expect("remove owned fixture");
    std::fs::remove_dir(directory).expect("no unexpected staging residue");
}

fn assert_maintenance_location(database: &Database) {
    assert_eq!(
        database
            .strings("SELECT `Signature_` FROM `AppSearch` WHERE `Property`='INSTALLDIR'")
            .expect("maintenance property"),
        ["InstallLocation"]
    );
    assert_eq!(database.strings("SELECT `Key` FROM `RegLocator` WHERE `Signature_`='InstallLocation' AND `Root`=1 AND `Type`=18 AND `Name`='InstallLocation'").expect("same user 64-bit registry lookup"), ["Software\\Metis\\Applications\\org.metis.package-test"]);
    assert_eq!(database.strings("SELECT `Value` FROM `Registry` WHERE `Registry`='InstallLocation' AND `Root`=1 AND `Name`='InstallLocation' AND `Component_`='C0'").expect("entry owns resolved installation path"), ["[INSTALLDIR]"]);
    assert_eq!(
        database
            .strings("SELECT `Key` FROM `Registry` WHERE `Registry`='InstallLocation'")
            .expect("storage and lookup agree"),
        ["Software\\Metis\\Applications\\org.metis.package-test"]
    );
    assert_eq!(
        database
            .strings("SELECT `Signature` FROM `Signature`")
            .expect("raw lookup has no file signature"),
        Vec::<String>::new()
    );
    for table in ["InstallUISequence", "InstallExecuteSequence"] {
        assert_eq!(
            database
                .strings(&format!(
                    "SELECT `Condition` FROM `{table}` WHERE `Action`='AppSearch' AND `Sequence`=50"
                ))
                .expect("maintenance search precedes launch checks and costing"),
            ["Installed"]
        );
        assert_eq!(
            database
                .strings(&format!(
                    "SELECT `Action` FROM `{table}` WHERE `Sequence`<=1000 ORDER BY `Sequence`"
                ))
                .expect("location search ordering"),
            [
                "FindRelatedProducts",
                "AppSearch",
                "LaunchConditions",
                "ValidateProductID",
                "CostInitialize",
                "FileCost",
                "CostFinalize"
            ]
        );
    }
    assert_eq!(database.strings("SELECT `Condition` FROM `LaunchCondition` WHERE `Condition`='NOT Installed OR INSTALLDIR'").expect("missing location rejects before default costing"), ["NOT Installed OR INSTALLDIR"]);
}

fn shortcut_path(database: &Database) -> String {
    let directories = database
        .strings("SELECT `DefaultDir` FROM `Directory` WHERE `Directory`='APPLICATIONMENU'")
        .expect("menu directory");
    let names = database
        .strings("SELECT `Name` FROM `Shortcut` WHERE `Directory_`='APPLICATIONMENU'")
        .expect("shortcut name");
    let [directory] = directories.as_slice() else {
        panic!("exactly one application menu directory is required")
    };
    let [name] = names.as_slice() else {
        panic!("exactly one shortcut must use the application menu directory")
    };
    format!(
        "{}/{}",
        directory.split_once('|').expect("long menu directory").1,
        name.split_once('|').expect("long shortcut name").1
    )
}

#[test]
fn unrepresentable_metadata_is_rejected_before_persistence() {
    use super::database::Value::Text;
    let directory =
        std::env::temp_dir().join(format!("metis-encoding-test-{}", guid().expect("OS GUID")));
    std::fs::create_dir(&directory).expect("isolated encoding test directory");
    let mut actual = Vec::new();
    for (index, value) in ["患者", "scan😀.txt", "∞"].into_iter().enumerate() {
        let path = directory.join(format!("encoding-{index}.msi"));
        let result = (|| -> Result<Vec<String>, Box<dyn std::error::Error>> {
            {
                let database = Database::create(&path)?;
                database.codepage(&directory)?;
                database.execute("CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(0) NOT NULL LOCALIZABLE PRIMARY KEY `Property`)", &[])?;
                database.execute(
                    "INSERT INTO `Property` (`Property`,`Value`) VALUES (?,?)",
                    &[Text("Label"), Text(value)],
                )?;
                database.commit()?;
            }
            Database::open(&path)?.strings("SELECT `Value` FROM `Property`")
        })();
        actual.push(result.map_err(|error| error.to_string()));
        if path.try_exists().expect("encoding database state") {
            std::fs::remove_file(path).expect("remove owned encoding database");
        }
        {
            let database = Database::create(&directory.join(format!("summary-{index}.msi")))
                .expect("summary database");
            actual.push(
                database
                    .summary(&guid().expect("package code"), value)
                    .map(|()| Vec::new())
                    .map_err(|error| error.to_string()),
            );
        }
    }
    std::fs::remove_file(directory.join("codepage.idt")).expect("remove owned codepage file");
    std::fs::remove_dir(directory).expect("encoding test cleanup");
    assert_eq!(
        actual,
        vec![Err("MSI metadata cannot be represented losslessly in Windows-1252".to_owned()); 6]
    );
}
