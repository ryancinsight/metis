//! Narrow Windows Installer and GUID ABI; declarations match Windows SDK 26100.
use std::ffi::c_void;

#[repr(C)]
#[derive(Default)]
pub(super) struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[link(name = "ole32")]
unsafe extern "system" {
    pub(super) fn CoCreateGuid(guid: *mut Guid) -> i32;
}

#[link(name = "msi")]
unsafe extern "system" {
    pub(super) fn MsiCloseHandle(handle: u32) -> u32;
    pub(super) fn MsiOpenDatabaseW(path: *const u16, mode: *const u16, result: *mut u32) -> u32;
    pub(super) fn MsiDatabaseOpenViewW(database: u32, query: *const u16, result: *mut u32) -> u32;
    pub(super) fn MsiViewExecute(view: u32, record: u32) -> u32;
    pub(super) fn MsiViewFetch(view: u32, record: *mut u32) -> u32;
    pub(super) fn MsiDatabaseCommit(database: u32) -> u32;
    pub(super) fn MsiDatabaseImportW(
        database: u32,
        folder: *const u16,
        filename: *const u16,
    ) -> u32;
    pub(super) fn MsiCreateRecord(fields: u32) -> u32;
    pub(super) fn MsiRecordSetStringW(record: u32, field: u32, value: *const u16) -> u32;
    pub(super) fn MsiRecordSetInteger(record: u32, field: u32, value: i32) -> u32;
    pub(super) fn MsiRecordSetStreamW(record: u32, field: u32, path: *const u16) -> u32;
    pub(super) fn MsiRecordGetStringW(
        record: u32,
        field: u32,
        value: *mut u16,
        length: *mut u32,
    ) -> u32;
    pub(super) fn MsiGetSummaryInformationW(
        database: u32,
        path: *const u16,
        updates: u32,
        result: *mut u32,
    ) -> u32;
    pub(super) fn MsiSummaryInfoSetPropertyW(
        summary: u32,
        property: u32,
        kind: u32,
        number: i32,
        time: *const c_void,
        text: *const u16,
    ) -> u32;
    pub(super) fn MsiSummaryInfoPersist(summary: u32) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub(super) fn GetSystemDirectoryW(buffer: *mut u16, length: u32) -> u32;
    pub(super) fn WideCharToMultiByte(
        code_page: u32,
        flags: u32,
        input: *const u16,
        input_length: i32,
        output: *mut u8,
        output_length: i32,
        default_character: *const u8,
        used_default: *mut i32,
    ) -> i32;
}
