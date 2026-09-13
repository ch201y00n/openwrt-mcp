//! ADR 0008/0019 SDK bridge. No handles or native types leave this module.
#![allow(unsafe_code)]

use super::policy;
use crate::HostError;
use std::{
    fs::File,
    io::{Read, Write},
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF,
            FILE_OPEN_REPARSE_POINT, FILE_RENAME_INFORMATION, FILE_SYNCHRONOUS_IO_NONALERT,
            FileRenameInformation, NtCreateFile, NtSetInformationFile,
        },
    },
    Win32::{
        Foundation::{
            ERROR_NO_TOKEN, GetLastError, HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE,
            STATUS_OBJECT_NAME_NOT_FOUND, UNICODE_STRING,
        },
        Security::{
            DACL_SECURITY_INFORMATION, GetKernelObjectSecurity, GetTokenInformation,
            OWNER_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        Storage::FileSystem::{
            DELETE, FILE_APPEND_DATA, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_OFFLINE,
            FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_DISPOSITION_INFO,
            FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_SHARE_READ, FILE_SHARE_WRITE,
            FILE_STANDARD_INFO, FILE_TRAVERSE, FILE_TYPE_DISK, FileAttributeTagInfo,
            FileDispositionInfo, FileStandardInfo, GetFileInformationByHandleEx, GetFileType,
            GetFinalPathNameByHandleW, GetVolumeInformationByHandleW, QueryDosDeviceW,
            READ_CONTROL, SYNCHRONIZE, SetFileInformationByHandle, VOLUME_NAME_GUID,
            VOLUME_NAME_NT,
        },
        System::{
            IO::IO_STATUS_BLOCK,
            Threading::{GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken},
        },
    },
};
use zeroize::Zeroizing;

fn owned(raw: HANDLE) -> Result<OwnedHandle, HostError> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(HostError::Unavailable);
    }
    // SAFETY: callers transfer a unique, successful SDK-created handle here once.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

fn open(
    parent: Option<&OwnedHandle>,
    name: &str,
    directory: bool,
) -> Result<OwnedHandle, HostError> {
    let mut wide: Vec<_> = name.encode_utf16().collect();
    let length = u16::try_from(wide.len() * 2).map_err(|_| HostError::InvalidPath)?;
    let mut unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: wide.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.map_or(null_mut(), AsRawHandle::as_raw_handle),
        ObjectName: &mut unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: null_mut(),
        SecurityQualityOfService: null_mut(),
    };
    let mut status = IO_STATUS_BLOCK::default();
    let mut raw = null_mut();
    // SAFETY: names/attributes/status are initialized and live for this synchronous call.
    // FILE_OPEN cannot create/overwrite; relative names were validated one component at a time.
    // No backup intent, write authority, delete sharing, or caller-controlled options.
    // Directory traversal participates in sharing checks: a metadata-only directory
    // handle did not exclude DELETE opens in native fixtures. Do not remove this
    // right merely because traversal can be bypassed by a process privilege.
    let result = unsafe {
        NtCreateFile(
            &mut raw,
            READ_CONTROL
                | FILE_READ_ATTRIBUTES
                | SYNCHRONIZE
                | if directory {
                    FILE_TRAVERSE
                } else {
                    FILE_READ_DATA
                },
            &attributes,
            &mut status,
            null(),
            0,
            FILE_SHARE_READ | if directory { FILE_SHARE_WRITE } else { 0 },
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT
                | FILE_SYNCHRONOUS_IO_NONALERT
                | if directory {
                    FILE_DIRECTORY_FILE
                } else {
                    FILE_NON_DIRECTORY_FILE
                },
            null(),
            0,
        )
    };
    if result < 0 {
        return Err(HostError::Unavailable);
    }
    owned(raw)
}

fn token_user() -> Result<Vec<u8>, HostError> {
    let mut raw = null_mut();
    // SAFETY: process/thread pseudo handles are borrowed, output handle storage is initialized.
    let impersonating = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut raw) };
    if impersonating != 0 {
        drop(owned(raw)?);
        return Err(HostError::Insecure);
    }
    // SAFETY: GetLastError reads this thread's immediately preceding SDK failure.
    if unsafe { GetLastError() } != ERROR_NO_TOKEN {
        return Err(HostError::Insecure);
    }
    // SAFETY: same initialized output, read-only process token request; no privilege changes.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(HostError::Unavailable);
    }
    let handle = owned(raw)?;
    // Pointer-bearing TOKEN_USER requires pointer alignment; 4096 initialized bytes suffice
    // for this bounded profile. Larger tokens fail rather than cause an unbounded retry.
    let mut words = [0usize; 4096 / size_of::<usize>()];
    let mut needed = 0u32;
    // SAFETY: aligned writable buffer has exactly 4096 live initialized bytes.
    if unsafe {
        GetTokenInformation(
            handle.as_raw_handle(),
            TokenUser,
            words.as_mut_ptr().cast(),
            4096,
            &mut needed,
        )
    } == 0
        || (needed as usize) < size_of::<TOKEN_USER>()
        || needed > 4096
    {
        return Err(HostError::Unavailable);
    }
    // SAFETY: the successful SDK call initialized a correctly aligned TOKEN_USER prefix.
    let user = unsafe { &*words.as_ptr().cast::<TOKEN_USER>() };
    let at = (user.User.Sid as usize)
        .checked_sub(words.as_ptr() as usize)
        .ok_or(HostError::Insecure)?;
    if at < size_of::<TOKEN_USER>() || at >= needed as usize {
        return Err(HostError::Insecure);
    }
    // SAFETY: byte aliasing is valid; length is bounded by the initialized allocation.
    let bytes = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), needed as usize) };
    Ok(policy::sid(&bytes[at..])?.to_vec())
}

fn security(handle: HANDLE) -> Result<Vec<u8>, HostError> {
    // DWORD aligned initialized buffer: the SDK returns a self-relative descriptor,
    // not pointers that can outlive this storage. No raw descriptor is logged.
    let mut words = vec![0u32; policy::MAX_DESCRIPTOR / 4];
    let mut needed = 0u32;
    // SAFETY: buffer alignment/length match the provided writable byte count.
    if unsafe {
        GetKernelObjectSecurity(
            handle,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            words.as_mut_ptr().cast(),
            policy::MAX_DESCRIPTOR as u32,
            &mut needed,
        )
    } == 0
        || needed < 20
        || needed as usize > policy::MAX_DESCRIPTOR
    {
        return Err(HostError::Insecure);
    }
    // SAFETY: returned range lies within initialized u32 storage; copying breaks all borrows.
    Ok(
        unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), needed as usize) }
            .to_vec(),
    )
}

fn final_path(handle: HANDLE) -> Result<String, HostError> {
    named_path(handle, VOLUME_NAME_GUID)
}

fn named_path(handle: HANDLE, kind: u32) -> Result<String, HostError> {
    let mut wide = vec![0u16; policy::MAX_PATH + 64];
    // SAFETY: valid borrowed file handle and initialized buffer/size; normalized GUID result.
    let count =
        unsafe { GetFinalPathNameByHandleW(handle, wide.as_mut_ptr(), wide.len() as u32, kind) }
            as usize;
    if count == 0 || count >= wide.len() {
        return Err(HostError::Unsupported);
    }
    String::from_utf16(&wide[..count]).map_err(|_| HostError::InvalidPath)
}

fn volume(handle: HANDLE, drive: char) -> Result<String, HostError> {
    let name = [drive as u16, ':' as u16, 0];
    let mut mapping = [0u16; 1024];
    // SAFETY: exact validated drive name, not device enumeration; bounded initialized output.
    let count =
        unsafe { QueryDosDeviceW(name.as_ptr(), mapping.as_mut_ptr(), mapping.len() as u32) }
            as usize;
    if count == 0 || count > mapping.len() {
        return Err(HostError::Unsupported);
    }
    let end = mapping[..count]
        .iter()
        .position(|v| *v == 0)
        .ok_or(HostError::Unsupported)?;
    let current = String::from_utf16(&mapping[..end]).map_err(|_| HostError::Unsupported)?;
    policy::volume_mapping(&current)?;
    policy::same_path(
        &format!("{current}\\"),
        &named_path(handle, VOLUME_NAME_NT)?,
    )?;
    let mut filesystem = [0u16; 32];
    let mut flags = 0u32;
    // SAFETY: optional outputs are null; live buffer and flag output have correct sizes.
    if unsafe {
        GetVolumeInformationByHandleW(
            handle,
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            &mut flags,
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    } == 0
        || filesystem[..5] != [78, 84, 70, 83, 0]
        || flags & 8 == 0
    {
        return Err(HostError::Unsupported);
    }
    let path = final_path(handle)?;
    let guid = path
        .strip_prefix("\\\\?\\Volume{")
        .and_then(|p| p.strip_suffix("}\\"))
        .ok_or(HostError::Unsupported)?;
    if guid.len() != 36
        || !guid.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err(HostError::Unsupported);
    }
    Ok(path)
}

fn inspect(
    handle: HANDLE,
    user: &[u8],
    directory: bool,
    root: bool,
    secret: bool,
    max: usize,
) -> Result<usize, HostError> {
    // SAFETY: caller holds a live owned handle throughout inspection and reading.
    if unsafe { GetFileType(handle) } != FILE_TYPE_DISK {
        return Err(HostError::Insecure);
    }
    let mut attributes = FILE_ATTRIBUTE_TAG_INFO::default();
    let mut standard = FILE_STANDARD_INFO::default();
    // SAFETY: initialized, correctly aligned output structs match their information classes.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileAttributeTagInfo,
            (&mut attributes as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    } == 0
        || unsafe {
            GetFileInformationByHandleEx(
                handle,
                FileStandardInfo,
                (&mut standard as *mut FILE_STANDARD_INFO).cast(),
                size_of::<FILE_STANDARD_INFO>() as u32,
            )
        } == 0
    {
        return Err(HostError::Insecure);
    }
    if attributes.FileAttributes
        & (FILE_ATTRIBUTE_REPARSE_POINT
            | FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
        || attributes.ReparseTag != 0
        || standard.Directory != directory
        || standard.DeletePending
        || (!directory && standard.NumberOfLinks != 1)
    {
        return Err(HostError::Insecure);
    }
    policy::descriptor(&security(handle)?, user, directory, root, secret)?;
    if !directory && (standard.EndOfFile < 0 || standard.EndOfFile as u64 > max as u64) {
        return Err(HostError::Limit);
    }
    Ok(if directory {
        0
    } else {
        standard.EndOfFile as usize
    })
}

pub(super) fn read(
    path: &Path,
    max_bytes: usize,
    secret: bool,
) -> Result<Zeroizing<Vec<u8>>, HostError> {
    let parsed = policy::path(path.to_str().ok_or(HostError::InvalidPath)?)?;
    let user = token_user()?;
    let root = open(None, &format!("\\??\\{}:\\", parsed.drive), true)?;
    let mut expected = volume(root.as_raw_handle(), parsed.drive)?;
    inspect(root.as_raw_handle(), &user, true, true, false, 0)?;
    let mut parents = vec![root];
    for (i, part) in parsed.components.iter().enumerate() {
        let directory = i + 1 != parsed.components.len();
        let handle = open(parents.last(), part, directory)?;
        let length = inspect(
            handle.as_raw_handle(),
            &user,
            directory,
            false,
            secret,
            max_bytes,
        )?;
        expected.push_str(part);
        policy::same_path(&expected, &final_path(handle.as_raw_handle())?)?;
        if directory {
            expected.push('\\');
            parents.push(handle);
            continue;
        }
        let mut file = File::from(handle);
        let mut bytes = Zeroizing::new(Vec::with_capacity(length.saturating_add(1)));
        (&mut file)
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| HostError::Unavailable)?;
        let after = inspect(file.as_raw_handle(), &user, false, false, secret, max_bytes)?;
        if bytes.len() > max_bytes {
            return Err(HostError::Limit);
        }
        if bytes.len() != length || after != length {
            return Err(HostError::Insecure);
        }
        return Ok(bytes);
    }
    Err(HostError::InvalidPath)
}

const LOG_HARD_BYTES: usize = 1_073_741_824;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogOpen {
    ExistingGeneration,
    InitialActive,
    NewActive,
}

fn log_child(
    parent: &OwnedHandle,
    name: &str,
    user: &[u8],
    mode: LogOpen,
) -> Result<Option<OwnedHandle>, HostError> {
    let mut wide: Vec<_> = name.encode_utf16().collect();
    if wide.is_empty() || wide.len() > 255 {
        return Err(HostError::InvalidPath);
    }
    let length = (wide.len() * 2) as u16;
    let mut unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: wide.as_mut_ptr(),
    };
    let active = mode != LogOpen::ExistingGeneration;
    let mut descriptor = if active {
        Some(policy::log_descriptor(user)?)
    } else {
        None
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &mut unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: descriptor
            .as_mut()
            .map_or(null_mut(), |v| v.as_mut_ptr().cast()),
        SecurityQualityOfService: null_mut(),
    };
    let mut status = IO_STATUS_BLOCK::default();
    let mut raw = null_mut();
    // SAFETY: synchronous call with initialized live name/descriptor/output storage.
    // A fixed single component, no overwrite, write-data, delete sharing or traversal.
    // Creation uses a validated protected DACL; an existing object's ACL is untouched.
    let result = unsafe {
        NtCreateFile(
            &mut raw,
            READ_CONTROL
                | FILE_READ_ATTRIBUTES
                | SYNCHRONIZE
                | DELETE
                | if active { FILE_APPEND_DATA } else { 0 },
            &attributes,
            &mut status,
            null(),
            FILE_ATTRIBUTE_NORMAL,
            FILE_SHARE_READ,
            match mode {
                LogOpen::ExistingGeneration => FILE_OPEN,
                LogOpen::InitialActive => FILE_OPEN_IF,
                LogOpen::NewActive => FILE_CREATE,
            },
            FILE_NON_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            null(),
            0,
        )
    };
    if result == STATUS_OBJECT_NAME_NOT_FOUND && mode == LogOpen::ExistingGeneration {
        return Ok(None);
    }
    if result < 0 {
        return Err(HostError::Unavailable);
    }
    Ok(Some(owned(raw)?))
}

fn rename_log(handle: HANDLE, parent: &OwnedHandle, name: &str) -> Result<(), HostError> {
    let wide: Vec<_> = name.encode_utf16().collect();
    if wide.is_empty() || wide.len() > 255 {
        return Err(HostError::InvalidPath);
    }
    // Pointer-aligned initialized storage includes the SDK header, complete name
    // and a zero terminator. Names are generated single components, never paths.
    let size = size_of::<FILE_RENAME_INFORMATION>() + (wide.len() + 1) * 2;
    let mut words = vec![0usize; size.div_ceil(size_of::<usize>())];
    let info = words.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    // SAFETY: the raw pointer retains provenance of the entire aligned allocation,
    // including its variable tail. Fixed SDK fields and the disjoint name fit in it.
    unsafe {
        (*info).Anonymous.ReplaceIfExists = false;
        (*info).RootDirectory = parent.as_raw_handle();
        (*info).FileNameLength = (wide.len() * 2) as u32;
        let destination = (&raw mut (*info).FileName).cast::<u16>();
        std::ptr::copy_nonoverlapping(wide.as_ptr(), destination, wide.len());
    }
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: valid synchronous owned handles and live initialized SDK buffers.
    // Use the native relative-name contract directly; no Win32 path translation,
    // replacement, bypass-access-check class, or caller-controlled flags.
    if unsafe {
        NtSetInformationFile(
            handle,
            &mut status,
            words.as_ptr().cast(),
            size as u32,
            FileRenameInformation,
        )
    } < 0
    {
        return Err(HostError::Unavailable);
    }
    Ok(())
}

fn delete_log(handle: HANDLE) -> Result<(), HostError> {
    let mut info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the caller holds the exact validated oldest file with DELETE access.
    // No name lookup, replace, readonly override or POSIX deletion option is used.
    if unsafe {
        SetFileInformationByHandle(
            handle,
            FileDispositionInfo,
            (&mut info as *mut FILE_DISPOSITION_INFO).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(HostError::Unavailable);
    }
    Ok(())
}

struct NativeLog {
    parents: Vec<OwnedHandle>,
    components: Vec<String>,
    user: Vec<u8>,
    drive: char,
    volume_root: String,
    prefix: String,
    name: String,
    file: File,
    size: u64,
    max_bytes: u64,
    retained: usize,
}

impl NativeLog {
    fn parent(&self) -> Result<&OwnedHandle, HostError> {
        self.parents.last().ok_or(HostError::Insecure)
    }

    fn check_file(&self, handle: HANDLE, name: &str) -> Result<u64, HostError> {
        let length = inspect(handle, &self.user, false, false, true, LOG_HARD_BYTES)?;
        policy::same_path(&format!("{}{name}", self.prefix), &final_path(handle)?)?;
        Ok(length as u64)
    }

    fn validate(&self) -> Result<(), HostError> {
        if token_user()? != self.user {
            return Err(HostError::Insecure);
        }
        let root = self.parents.first().ok_or(HostError::Insecure)?;
        policy::same_path(
            &self.volume_root,
            &volume(root.as_raw_handle(), self.drive)?,
        )?;
        let mut expected = self.volume_root.clone();
        for (index, handle) in self.parents.iter().enumerate() {
            inspect(
                handle.as_raw_handle(),
                &self.user,
                true,
                index == 0,
                false,
                0,
            )?;
            if index != 0 {
                expected.push_str(&self.components[index - 1]);
                policy::same_path(&expected, &final_path(handle.as_raw_handle())?)?;
                expected.push('\\');
            }
        }
        if self.check_file(self.file.as_raw_handle(), &self.name)? != self.size {
            return Err(HostError::Insecure);
        }
        Ok(())
    }

    fn generation(&self, index: usize) -> String {
        format!("{}.{index}", self.name)
    }

    fn rotate(&mut self) -> Result<(), HostError> {
        let mut generations = Vec::with_capacity(self.retained);
        // Hold every existing object before any deletion/rename. Unknown is not absent.
        for index in 1..=self.retained {
            let name = self.generation(index);
            let handle = log_child(
                self.parent()?,
                &name,
                &self.user,
                LogOpen::ExistingGeneration,
            )?;
            if let Some(handle) = &handle {
                self.check_file(handle.as_raw_handle(), &name)?;
            }
            generations.push(handle);
        }
        self.validate()?;
        if let Some(oldest) = generations.last_mut().and_then(Option::take) {
            delete_log(oldest.as_raw_handle())?;
            drop(oldest);
        }
        for index in (1..self.retained).rev() {
            if let Some(handle) = &generations[index - 1] {
                self.check_file(handle.as_raw_handle(), &self.generation(index))?;
                let destination = self.generation(index + 1);
                rename_log(handle.as_raw_handle(), self.parent()?, &destination)?;
                self.check_file(handle.as_raw_handle(), &destination)?;
            }
        }
        let destination = self.generation(1);
        rename_log(self.file.as_raw_handle(), self.parent()?, &destination)?;
        self.check_file(self.file.as_raw_handle(), &destination)?;
        let handle = log_child(self.parent()?, &self.name, &self.user, LogOpen::NewActive)?
            .ok_or(HostError::Unavailable)?;
        if self.check_file(handle.as_raw_handle(), &self.name)? != 0 {
            return Err(HostError::Insecure);
        }
        self.file = File::from(handle);
        self.size = 0;
        Ok(())
    }
}

impl super::log::LogWriter for NativeLog {
    fn write(&mut self, bytes: &[u8]) -> Result<(), HostError> {
        if bytes.len() as u64 > self.max_bytes {
            return Err(HostError::Limit);
        }
        self.validate()?;
        if bytes.is_empty() {
            return Ok(());
        }
        if self
            .size
            .checked_add(bytes.len() as u64)
            .ok_or(HostError::Limit)?
            > self.max_bytes
        {
            self.rotate()?;
        }
        for part in bytes.chunks(65_536) {
            self.file
                .write_all(part)
                .map_err(|_| HostError::Unavailable)?;
        }
        self.size = self
            .size
            .checked_add(bytes.len() as u64)
            .ok_or(HostError::Limit)?;
        self.validate()
    }
}

pub(super) fn open_log(
    path: &Path,
    max_bytes: u64,
    retained: usize,
) -> Result<Box<dyn super::log::LogWriter>, HostError> {
    if max_bytes == 0 || max_bytes > LOG_HARD_BYTES as u64 || !(1..=100).contains(&retained) {
        return Err(HostError::Limit);
    }
    let text = path.to_str().ok_or(HostError::InvalidPath)?;
    let parsed = policy::path(text)?;
    // Validate all derived names before even a new empty active log can be created.
    for index in 1..=retained {
        policy::path(&format!("{text}.{index}"))?;
    }
    let name = parsed
        .components
        .last()
        .ok_or(HostError::InvalidPath)?
        .to_string();
    let user = token_user()?;
    let root = open(None, &format!("\\??\\{}:\\", parsed.drive), true)?;
    let volume_root = volume(root.as_raw_handle(), parsed.drive)?;
    inspect(root.as_raw_handle(), &user, true, true, false, 0)?;
    let mut prefix = volume_root.clone();
    let mut parents = vec![root];
    let mut components = Vec::new();
    for part in parsed.components.iter().take(parsed.components.len() - 1) {
        let handle = open(parents.last(), part, true)?;
        inspect(handle.as_raw_handle(), &user, true, false, false, 0)?;
        prefix.push_str(part);
        policy::same_path(&prefix, &final_path(handle.as_raw_handle())?)?;
        prefix.push('\\');
        parents.push(handle);
        components.push((*part).to_owned());
    }
    let handle = log_child(
        parents.last().ok_or(HostError::Insecure)?,
        &name,
        &user,
        LogOpen::InitialActive,
    )?
    .ok_or(HostError::Unavailable)?;
    let size = inspect(
        handle.as_raw_handle(),
        &user,
        false,
        false,
        true,
        LOG_HARD_BYTES,
    )? as u64;
    policy::same_path(
        &format!("{prefix}{name}"),
        &final_path(handle.as_raw_handle())?,
    )?;
    Ok(Box::new(NativeLog {
        parents,
        components,
        user,
        drive: parsed.drive,
        volume_root,
        prefix,
        name,
        file: File::from(handle),
        size,
        max_bytes,
        retained,
    }))
}
