//! ADR 0008 audited SDK bridge. No handles or native types leave this module.
#![allow(unsafe_code)]

use super::policy;
use crate::HostError;
use std::{
    fs::File,
    io::Read,
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
            FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
        },
    },
    Win32::{
        Foundation::{
            ERROR_NO_TOKEN, GetLastError, HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE,
            UNICODE_STRING,
        },
        Security::{
            DACL_SECURITY_INFORMATION, GetKernelObjectSecurity, GetTokenInformation,
            OWNER_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        Storage::FileSystem::{
            FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
            FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
            FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_SHARE_READ, FILE_SHARE_WRITE,
            FILE_STANDARD_INFO, FILE_TYPE_DISK, FileAttributeTagInfo, FileStandardInfo,
            GetFileInformationByHandleEx, GetFileType, GetFinalPathNameByHandleW,
            GetVolumeInformationByHandleW, QueryDosDeviceW, READ_CONTROL, SYNCHRONIZE,
            VOLUME_NAME_GUID, VOLUME_NAME_NT,
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
    let result = unsafe {
        NtCreateFile(
            &mut raw,
            READ_CONTROL
                | FILE_READ_ATTRIBUTES
                | SYNCHRONIZE
                | if directory { 0 } else { FILE_READ_DATA },
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
