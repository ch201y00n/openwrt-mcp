use super::{ArchiveError, BLOCK_BYTES, MAX_FILE_BYTES, MAX_PATH_BYTES, manifest::validate_path};
use zeroize::Zeroizing;

pub(super) struct Header {
    pub path: Zeroizing<Vec<u8>>,
    pub size: u64,
}

fn octal(field: &[u8]) -> Result<u64, ArchiveError> {
    let start = field
        .iter()
        .position(|b| *b != b' ')
        .ok_or(ArchiveError::InvalidHeader)?;
    let mut end = start;
    let mut value = 0_u64;
    while end < field.len() && matches!(field[end], b'0'..=b'7') {
        value = value
            .checked_mul(8)
            .and_then(|n| n.checked_add(u64::from(field[end] - b'0')))
            .ok_or(ArchiveError::InvalidHeader)?;
        end += 1;
    }
    if end == start || !field[end..].iter().all(|b| matches!(b, b' ' | 0)) {
        return Err(ArchiveError::InvalidHeader);
    }
    Ok(value)
}

fn text(field: &[u8]) -> Result<&[u8], ArchiveError> {
    if let Some(end) = field.iter().position(|b| *b == 0) {
        if field[end..].iter().any(|b| *b != 0) {
            return Err(ArchiveError::InvalidHeader);
        }
        Ok(&field[..end])
    } else {
        Ok(field)
    }
}

pub(super) fn parse(block: &[u8; BLOCK_BYTES]) -> Result<Header, ArchiveError> {
    let checksum = octal(&block[148..156])?;
    let sum: u64 = block
        .iter()
        .enumerate()
        .map(|(i, b)| u64::from(if (148..156).contains(&i) { b' ' } else { *b }))
        .sum();
    if sum != checksum {
        return Err(ArchiveError::InvalidHeader);
    }
    let prefix = match &block[257..265] {
        b"ustar\x0000" => {
            if block[500..].iter().any(|b| *b != 0) {
                return Err(ArchiveError::InvalidHeader);
            }
            text(&block[345..500])?
        }
        b"ustar  \0" => {
            if block[345..].iter().any(|b| *b != 0) {
                return Err(ArchiveError::InvalidHeader);
            }
            &[][..]
        }
        _ => return Err(ArchiveError::InvalidHeader),
    };
    if !matches!(block[156], b'0' | 0) {
        return Err(ArchiveError::UnsupportedEntry);
    }
    if block[157..257].iter().any(|b| *b != 0)
        || octal(&block[100..108])? > 0o777
        || octal(&block[108..116])? > u64::from(u32::MAX)
        || octal(&block[116..124])? > u64::from(u32::MAX)
    {
        return Err(ArchiveError::InvalidHeader);
    }
    let size = octal(&block[124..136])?;
    if size > MAX_FILE_BYTES {
        return Err(ArchiveError::LimitExceeded);
    }
    octal(&block[136..148])?;
    for field in [&block[329..337], &block[337..345]] {
        if !field.iter().all(|b| *b == 0) && octal(field)? != 0 {
            return Err(ArchiveError::InvalidHeader);
        }
    }
    for field in [&block[265..297], &block[297..329]] {
        if !text(field)?.iter().all(u8::is_ascii_graphic) {
            return Err(ArchiveError::InvalidHeader);
        }
    }
    let name = text(&block[..100])?;
    let length = prefix.len() + usize::from(!prefix.is_empty()) + name.len();
    if length > MAX_PATH_BYTES || name.is_empty() {
        return Err(ArchiveError::InvalidPath);
    }
    let mut path = Zeroizing::new(Vec::with_capacity(length));
    path.extend_from_slice(prefix);
    if !prefix.is_empty() {
        path.push(b'/');
    }
    path.extend_from_slice(name);
    validate_path(&path)?;
    Ok(Header { path, size })
}
