//! Owned MSI handles and parameterized records keep package metadata out of SQL.
use super::{encoding, ffi, paths};
use std::{error::Error, ffi::OsStr, os::windows::ffi::OsStrExt, path::Path, ptr};

pub(super) struct Handle(u32);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: This is the sole owner of a nonzero MSI handle, on its creating
        // thread. Closing it releases only native bookkeeping and never waits.
        let status = unsafe { ffi::MsiCloseHandle(self.0) };
        if status != 0 {
            // A failed close is a programmer error; Drop cannot unwind safely.
            std::process::abort();
        }
    }
}

pub(super) fn check(status: u32, operation: &str) -> Result<(), Box<dyn Error>> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("Windows Installer {operation} failed: {status}").into())
    }
}

fn wide(value: &OsStr) -> Result<Vec<u16>, Box<dyn Error>> {
    let mut encoded: Vec<_> = value.encode_wide().collect();
    if encoded.contains(&0) {
        return Err("Windows Installer text contains NUL".into());
    }
    encoded.push(0);
    Ok(encoded)
}

pub(super) enum Value<'a> {
    Text(&'a str),
    Number(i32),
    Null,
    Stream(&'a Path),
}

fn metadata(value: &str) -> Result<Vec<u16>, Box<dyn Error>> {
    let encoded = wide(OsStr::new(value))?;
    encoding::validate(&encoded)?;
    Ok(encoded)
}

pub(super) struct Database(Handle);

impl Database {
    pub(super) fn open(path: &Path) -> Result<Self, Box<dyn Error>> {
        let path = wide(paths::legacy(path)?.as_os_str())?;
        let mut handle = 0;
        // SAFETY: Path is terminated, result writable, null mode is MSI read-only.
        check(
            unsafe { ffi::MsiOpenDatabaseW(path.as_ptr(), ptr::null(), &raw mut handle) },
            "open database",
        )?;
        Ok(Self(Handle(handle)))
    }

    pub(super) fn strings(&self, sql: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let query = wide(OsStr::new(sql))?;
        let mut view = 0;
        // SAFETY: Query is terminated; database and writable handle storage live.
        check(
            unsafe { ffi::MsiDatabaseOpenViewW(self.0.0, query.as_ptr(), &raw mut view) },
            "open inspection query",
        )?;
        let view = Handle(view);
        // SAFETY: View lives; this fixed read query has no bound parameters.
        check(
            unsafe { ffi::MsiViewExecute(view.0, 0) },
            "execute inspection query",
        )?;
        let mut values = Vec::new();
        loop {
            let mut record = 0;
            // SAFETY: View is executing and result storage is writable.
            let status = unsafe { ffi::MsiViewFetch(view.0, &raw mut record) };
            if status == 259 {
                break;
            } // ERROR_NO_MORE_ITEMS is normal exhaustion.
            check(status, "fetch inspection row")?;
            let record = Handle(record);
            if values.len() >= 4096 {
                return Err("MSI inspection exceeds 4096 rows".into());
            }
            let mut encoded = [0_u16; 512];
            let mut length = u32::try_from(encoded.len())?;
            // SAFETY: Buffer is writable for length UTF-16 units; field 1 exists
            // in all static single-column queries used by the inspection API.
            check(
                unsafe {
                    ffi::MsiRecordGetStringW(record.0, 1, encoded.as_mut_ptr(), &raw mut length)
                },
                "read inspection field",
            )?;
            let value = encoded
                .get(..usize::try_from(length)?)
                .ok_or("MSI inspection field exceeds buffer")?;
            values.push(String::from_utf16(value)?);
        }
        Ok(values)
    }
    pub(super) fn create(path: &Path) -> Result<Self, Box<dyn Error>> {
        let path = wide(paths::legacy(path)?.as_os_str())?;
        let mut handle = 0;
        // SAFETY: Path is a terminated UTF-16 string and result is writable.
        // MSIDBOPEN_CREATE is the documented integer sentinel 3, never dereferenced.
        let status = unsafe {
            ffi::MsiOpenDatabaseW(path.as_ptr(), ptr::without_provenance(3), &raw mut handle)
        };
        check(status, "create database")?;
        Ok(Self(Handle(handle)))
    }

    pub(super) fn codepage(&self, folder: &Path) -> Result<(), Box<dyn Error>> {
        // Microsoft _ForceCodepage's text archive contract: two blank lines,
        // then the numeric code page, tab and literal table name.
        std::fs::write(
            folder.join("codepage.idt"),
            format!("\r\n\r\n{}\t_ForceCodepage\r\n", encoding::CODE_PAGE),
        )?;
        let folder = wide(paths::legacy(folder)?.as_os_str())?;
        let filename = wide(OsStr::new("codepage.idt"))?;
        // SAFETY: Live database and terminated folder/filename strings; the API
        // reads the owned file before any localized metadata is inserted.
        check(
            unsafe { ffi::MsiDatabaseImportW(self.0.0, folder.as_ptr(), filename.as_ptr()) },
            "set database code page",
        )
    }

    pub(super) fn execute(&self, sql: &str, fields: &[Value<'_>]) -> Result<(), Box<dyn Error>> {
        let operation = format!("prepare query {sql}");
        let sql = wide(OsStr::new(sql))?;
        let mut view = 0;
        // SAFETY: Database is live; query is terminated; result has writable storage.
        check(
            unsafe { ffi::MsiDatabaseOpenViewW(self.0.0, sql.as_ptr(), &raw mut view) },
            &operation,
        )?;
        let view = Handle(view);
        // SAFETY: Checked field count fits the SDK's UINT parameter.
        let record = unsafe { ffi::MsiCreateRecord(u32::try_from(fields.len())?) };
        if record == 0 {
            return Err("Windows Installer cannot allocate record".into());
        }
        let record = Handle(record);
        for (index, value) in fields.iter().enumerate() {
            let index = u32::try_from(index + 1)?;
            let status = match value {
                Value::Text(text) => {
                    let text = metadata(text)?;
                    // SAFETY: Record and field exist; text remains live for this call.
                    unsafe { ffi::MsiRecordSetStringW(record.0, index, text.as_ptr()) }
                }
                Value::Number(number) => {
                    // SAFETY: Record and field exist; integer has the SDK's signed width.
                    unsafe { ffi::MsiRecordSetInteger(record.0, index, *number) }
                }
                Value::Null => continue,
                Value::Stream(path) => {
                    let path = wide(paths::legacy(path)?.as_os_str())?;
                    // SAFETY: Record and field exist; source path is terminated and live.
                    unsafe { ffi::MsiRecordSetStreamW(record.0, index, path.as_ptr()) }
                }
            };
            check(status, "set record field")?;
        }
        // SAFETY: View and parameter record are live until execution returns.
        check(
            unsafe { ffi::MsiViewExecute(view.0, if fields.is_empty() { 0 } else { record.0 }) },
            "execute query",
        )
    }

    pub(super) fn summary(&self, package: &str, manufacturer: &str) -> Result<(), Box<dyn Error>> {
        let mut summary = 0;
        // SAFETY: Database is live; null path selects that database; result writable.
        check(
            unsafe { ffi::MsiGetSummaryInformationW(self.0.0, ptr::null(), 10, &raw mut summary) },
            "open summary",
        )?;
        let summary = Handle(summary);
        for (property, text) in [
            (2, "Installation Database"),
            (4, manufacturer),
            (7, "x64;1033"),
            (9, package),
            (18, "Metis"),
        ] {
            let text = metadata(text)?;
            // SAFETY: VT_LPSTR (30) takes the live UTF-16 value in this W API.
            check(
                unsafe {
                    ffi::MsiSummaryInfoSetPropertyW(
                        summary.0,
                        property,
                        30,
                        0,
                        ptr::null(),
                        text.as_ptr(),
                    )
                },
                "write summary text",
            )?;
        }
        // PID_WORDCOUNT = compressed + no elevation; PID_PAGECOUNT = MSI 5.0.
        for (property, kind, number) in [
            (1, 2, i32::try_from(encoding::CODE_PAGE)?),
            (14, 3, 500),
            (15, 3, 10),
            (19, 3, 2),
        ] {
            // SAFETY: VT_I2/VT_I4 select number, so time/text are null and ignored.
            check(
                unsafe {
                    ffi::MsiSummaryInfoSetPropertyW(
                        summary.0,
                        property,
                        kind,
                        number,
                        ptr::null(),
                        ptr::null(),
                    )
                },
                "write summary number",
            )?;
        }
        // SAFETY: Summary is live and owns the pending property changes.
        check(
            unsafe { ffi::MsiSummaryInfoPersist(summary.0) },
            "persist summary",
        )
    }

    pub(super) fn commit(&self) -> Result<(), Box<dyn Error>> {
        // SAFETY: Database is an open transaction on its creating thread.
        check(
            unsafe { ffi::MsiDatabaseCommit(self.0.0) },
            "commit database",
        )
    }
}

pub(super) fn guid() -> Result<String, Box<dyn Error>> {
    let mut value = ffi::Guid::default();
    // SAFETY: GUID repr(C) matches SDK layout and is writable for the whole call.
    let status = unsafe { ffi::CoCreateGuid(&raw mut value) };
    if status < 0 {
        return Err(format!("Create package GUID failed: {status}").into());
    }
    let ffi::Guid {
        data1,
        data2,
        data3,
        data4:
            [
                octet0,
                octet1,
                octet2,
                octet3,
                octet4,
                octet5,
                octet6,
                octet7,
            ],
    } = value;
    Ok(format!(
        "{{{data1:08X}-{data2:04X}-{data3:04X}-{octet0:02X}{octet1:02X}-{octet2:02X}{octet3:02X}{octet4:02X}{octet5:02X}{octet6:02X}{octet7:02X}}}"
    ))
}
