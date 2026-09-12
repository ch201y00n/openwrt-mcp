use crate::config::entry_name;
use openwrt_mcp_runtime::protection::{KeyContainer, KeyLimits, KeyMaterial, ProtectionError};
use std::collections::BTreeSet;
use std::io::{Cursor, Read};
use zeroize::Zeroizing;

pub struct ZipContainer;

impl KeyContainer for ZipContainer {
    fn read_entry(
        &self,
        container: &KeyMaterial,
        entry: &str,
        limits: &KeyLimits,
    ) -> Result<KeyMaterial, ProtectionError> {
        limits.validate()?;
        if !entry_name(entry, false) {
            return Err(ProtectionError::InvalidConfig);
        }
        let bytes = container.expose_bytes();
        if bytes.len() > limits.max_container_bytes {
            return Err(ProtectionError::ResourceLimit);
        }
        let selected = preflight(bytes, entry, limits)?;
        // Construct the general parser only AFTER bounding central-directory metadata/count.
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| ProtectionError::InvalidContainer)?;
        let mut file = archive
            .by_index(selected)
            .map_err(|_| ProtectionError::InvalidContainer)?;
        if file.name() != entry || !file.is_file() || file.is_symlink() || file.encrypted() {
            return Err(ProtectionError::InvalidContainer);
        }
        let compressed =
            usize::try_from(file.compressed_size()).map_err(|_| ProtectionError::ResourceLimit)?;
        let actual_limit = limits
            .max_key_bytes
            .min(compressed.saturating_mul(limits.max_expansion_ratio));
        let mut output = Zeroizing::new(Vec::with_capacity(actual_limit.saturating_add(1)));
        (&mut file)
            .take(actual_limit.saturating_add(1) as u64)
            .read_to_end(&mut output)
            .map_err(|_| ProtectionError::InvalidContainer)?;
        if output.len() > actual_limit {
            return Err(ProtectionError::ResourceLimit);
        }
        if output.len() as u64 != file.size() {
            return Err(ProtectionError::InvalidContainer);
        }
        // A selected nested ZIP is not key material; registry container chains are also forbidden.
        if output.starts_with(b"PK\x03\x04") || output.starts_with(b"PK\x05\x06") {
            return Err(ProtectionError::UnsupportedContainer);
        }
        KeyMaterial::from_zeroizing(output, limits.max_key_bytes)
    }
}

fn number16(bytes: &[u8], offset: usize) -> Result<usize, ProtectionError> {
    let pair = bytes
        .get(
            offset
                ..offset
                    .checked_add(2)
                    .ok_or(ProtectionError::InvalidContainer)?,
        )
        .ok_or(ProtectionError::InvalidContainer)?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]) as usize)
}

fn number32(bytes: &[u8], offset: usize) -> Result<usize, ProtectionError> {
    let four = bytes
        .get(
            offset
                ..offset
                    .checked_add(4)
                    .ok_or(ProtectionError::InvalidContainer)?,
        )
        .ok_or(ProtectionError::InvalidContainer)?;
    Ok(u32::from_le_bytes([four[0], four[1], four[2], four[3]]) as usize)
}

fn span(bytes: &[u8], start: usize, length: usize) -> Result<&[u8], ProtectionError> {
    bytes
        .get(
            start
                ..start
                    .checked_add(length)
                    .ok_or(ProtectionError::InvalidContainer)?,
        )
        .ok_or(ProtectionError::InvalidContainer)
}

fn preflight(bytes: &[u8], requested: &str, limits: &KeyLimits) -> Result<usize, ProtectionError> {
    if bytes.len() < 22 {
        return Err(ProtectionError::InvalidContainer);
    }
    let start = bytes.len().saturating_sub(65_557);
    let end = (start..=bytes.len() - 22)
        .rev()
        .find(|&p| {
            bytes.get(p..p + 4) == Some(b"PK\x05\x06")
                && number16(bytes, p + 20).is_ok_and(|length| p + 22 + length == bytes.len())
        })
        .ok_or(ProtectionError::InvalidContainer)?;
    // The upstream parser permits garbage after an EOCD's comment. Do not let a
    // second signature inside this comment select a different, unbounded directory.
    if bytes[end + 22..]
        .windows(4)
        .any(|value| value == b"PK\x05\x06")
    {
        return Err(ProtectionError::InvalidContainer);
    }
    let count = number16(bytes, end + 10)?;
    let directory_size = number32(bytes, end + 12)?;
    let directory_start = number32(bytes, end + 16)?;
    if number16(bytes, end + 4)? != 0
        || number16(bytes, end + 6)? != 0
        || number16(bytes, end + 8)? != count
        || count == 0xffff
        || directory_size == u32::MAX as usize
        || directory_start == u32::MAX as usize
    {
        return Err(ProtectionError::UnsupportedContainer);
    }
    if count > limits.max_entries || directory_size > limits.max_directory_bytes {
        return Err(ProtectionError::ResourceLimit);
    }
    if directory_start.checked_add(directory_size) != Some(end) {
        return Err(ProtectionError::InvalidContainer);
    }
    let mut position = directory_start;
    let mut names = BTreeSet::new();
    let mut ranges = Vec::new();
    let mut selected = None;
    for index in 0..count {
        if span(bytes, position, 4)? != b"PK\x01\x02" || position + 46 > end {
            return Err(ProtectionError::InvalidContainer);
        }
        let flags = number16(bytes, position + 8)?;
        let method = number16(bytes, position + 10)?;
        let crc = number32(bytes, position + 16)?;
        let compressed = number32(bytes, position + 20)?;
        let inflated = number32(bytes, position + 24)?;
        let name_len = number16(bytes, position + 28)?;
        let extra_len = number16(bytes, position + 30)?;
        let comment_len = number16(bytes, position + 32)?;
        let external = number32(bytes, position + 38)?;
        let local = number32(bytes, position + 42)?;
        if flags & !(0x0800 | 0x0008) != 0
            || !matches!(method, 0 | 8)
            || number16(bytes, position + 34)? != 0
            || compressed == u32::MAX as usize
            || inflated == u32::MAX as usize
            || local == u32::MAX as usize
        {
            return Err(ProtectionError::UnsupportedContainer);
        }
        let name_bytes = span(bytes, position + 46, name_len)?;
        let name =
            std::str::from_utf8(name_bytes).map_err(|_| ProtectionError::InvalidContainer)?;
        let directory = name.ends_with('/');
        let file_type = (external >> 16) & 0o170000;
        if !entry_name(name, directory)
            || !names.insert(name.trim_end_matches('/'))
            || !matches!(file_type, 0 | 0o100000 | 0o040000)
            || (file_type == 0o040000 && !directory)
            || (directory && inflated != 0)
        {
            return Err(ProtectionError::InvalidContainer);
        }
        let next = position
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or(ProtectionError::InvalidContainer)?;
        if next > end {
            return Err(ProtectionError::InvalidContainer);
        }
        validate_extra(span(bytes, position + 46 + name_len, extra_len)?)?;
        if name == requested {
            if directory {
                return Err(ProtectionError::InvalidContainer);
            }
            if inflated > limits.max_key_bytes
                || inflated > compressed.saturating_mul(limits.max_expansion_ratio)
                || (method == 0 && inflated != compressed)
            {
                return Err(ProtectionError::ResourceLimit);
            }
            selected = Some(index);
        }
        let expected = ExpectedEntry {
            name: name_bytes,
            flags,
            method,
            crc,
            compressed,
            inflated,
        };
        let region = validate_local(bytes, local, expected, directory_start)?;
        ranges.push(region);
        position = next;
    }
    if position != end {
        return Err(ProtectionError::InvalidContainer);
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(ProtectionError::InvalidContainer);
    }
    selected.ok_or(ProtectionError::SourceUnavailable)
}

fn validate_extra(bytes: &[u8]) -> Result<(), ProtectionError> {
    let mut position = 0;
    while position < bytes.len() {
        let tag = number16(bytes, position)?;
        let length = number16(bytes, position + 2)?;
        // ZIP64, AES and Unicode alternate-name metadata introduce unsupported ambiguity.
        if matches!(tag, 0x0001 | 0x9901 | 0x7075) {
            return Err(ProtectionError::UnsupportedContainer);
        }
        let next = position
            .checked_add(4 + length)
            .ok_or(ProtectionError::InvalidContainer)?;
        if next > bytes.len() {
            return Err(ProtectionError::InvalidContainer);
        }
        position = next;
    }
    Ok(())
}

struct ExpectedEntry<'a> {
    name: &'a [u8],
    flags: usize,
    method: usize,
    crc: usize,
    compressed: usize,
    inflated: usize,
}

fn validate_local(
    bytes: &[u8],
    local: usize,
    expected: ExpectedEntry<'_>,
    directory_start: usize,
) -> Result<(usize, usize), ProtectionError> {
    if span(bytes, local, 4)? != b"PK\x03\x04" || local + 30 > directory_start {
        return Err(ProtectionError::InvalidContainer);
    }
    let name_len = number16(bytes, local + 26)?;
    let extra_len = number16(bytes, local + 28)?;
    if number16(bytes, local + 6)? != expected.flags
        || number16(bytes, local + 8)? != expected.method
        || span(bytes, local + 30, name_len)? != expected.name
    {
        return Err(ProtectionError::InvalidContainer);
    }
    validate_extra(span(bytes, local + 30 + name_len, extra_len)?)?;
    let data = local
        .checked_add(30 + name_len + extra_len)
        .ok_or(ProtectionError::InvalidContainer)?;
    let mut end = data
        .checked_add(expected.compressed)
        .ok_or(ProtectionError::InvalidContainer)?;
    if expected.flags & 8 == 0 {
        if number32(bytes, local + 14)? != expected.crc
            || number32(bytes, local + 18)? != expected.compressed
            || number32(bytes, local + 22)? != expected.inflated
        {
            return Err(ProtectionError::InvalidContainer);
        }
    } else {
        for (offset, value) in [
            (14, expected.crc),
            (18, expected.compressed),
            (22, expected.inflated),
        ] {
            let local_value = number32(bytes, local + offset)?;
            if local_value != 0 && local_value != value {
                return Err(ProtectionError::InvalidContainer);
            }
        }
        let descriptor = if span(bytes, end, 4)? == b"PK\x07\x08" {
            end + 4
        } else {
            end
        };
        if number32(bytes, descriptor)? != expected.crc
            || number32(bytes, descriptor + 4)? != expected.compressed
            || number32(bytes, descriptor + 8)? != expected.inflated
        {
            return Err(ProtectionError::InvalidContainer);
        }
        end = descriptor
            .checked_add(12)
            .ok_or(ProtectionError::InvalidContainer)?;
    }
    if end > directory_start {
        return Err(ProtectionError::InvalidContainer);
    }
    Ok((local, end))
}
