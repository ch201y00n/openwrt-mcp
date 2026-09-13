//! Safe validation of supplied Windows path/security bytes. No SDK or host I/O.
use crate::HostError;

pub(super) const MAX_PATH: usize = 4096;
pub(super) const MAX_DESCRIPTOR: usize = 65536;

pub(super) fn volume_mapping(mapping: &str) -> Result<(), HostError> {
    let suffix = mapping
        .strip_prefix("\\Device\\HarddiskVolume")
        .ok_or(HostError::Unsupported)?;
    if suffix.is_empty() || suffix.len() > 10 || !suffix.bytes().all(|v| v.is_ascii_digit()) {
        return Err(HostError::Unsupported);
    }
    Ok(())
}

pub(super) fn same_path(expected: &str, observed: &str) -> Result<(), HostError> {
    if expected.eq_ignore_ascii_case(observed) {
        Ok(())
    } else {
        Err(HostError::Insecure)
    }
}

pub(super) struct DosPath<'a> {
    pub(super) drive: char,
    pub(super) components: Vec<&'a str>,
}

pub(super) fn path(value: &str) -> Result<DosPath<'_>, HostError> {
    let bytes = value.as_bytes();
    if value.encode_utf16().count() > MAX_PATH
        || bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1..3] != *b":\\"
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(HostError::InvalidPath);
    }
    let components: Vec<_> = value[3..].split('\\').collect();
    if components.len() > 128 {
        return Err(HostError::InvalidPath);
    }
    for part in &components {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let numbered = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"));
        if part.is_empty()
            || part.contains(':')
            || part.ends_with(['.', ' '])
            || part.encode_utf16().count() > 255
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            || numbered.is_some_and(|n| {
                matches!(
                    n,
                    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        {
            return Err(HostError::InvalidPath);
        }
    }
    Ok(DosPath {
        drive: char::from(bytes[0].to_ascii_uppercase()),
        components,
    })
}

pub(super) fn sid(bytes: &[u8]) -> Result<&[u8], HostError> {
    if bytes.len() < 8 || bytes[0] != 1 || !(1..=15).contains(&bytes[1]) {
        return Err(HostError::Insecure);
    }
    bytes
        .get(..8 + usize::from(bytes[1]) * 4)
        .ok_or(HostError::Insecure)
}

/// A small DWORD-aligned self-relative descriptor, private at file creation.
/// It grants only the exact validated process user, without inherited ACEs.
pub(super) fn log_descriptor(user: &[u8]) -> Result<Vec<u32>, HostError> {
    if sid(user)? != user {
        return Err(HostError::Insecure);
    }
    let mut bytes = vec![0_u8; 20];
    bytes[0] = 1;
    // SELF_RELATIVE | DACL_PROTECTED | DACL_PRESENT.
    bytes[2..4].copy_from_slice(&0x9004_u16.to_le_bytes());
    bytes[4..8].copy_from_slice(&20_u32.to_le_bytes());
    bytes.extend_from_slice(user);
    let acl = bytes.len();
    bytes[16..20].copy_from_slice(&(acl as u32).to_le_bytes());
    let acl_size = 16 + user.len();
    bytes.extend_from_slice(&[2, 0]);
    bytes.extend_from_slice(&(acl_size as u16).to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend_from_slice(&[0, 0]); // One non-inheritable ACCESS_ALLOWED_ACE.
    bytes.extend_from_slice(&((8 + user.len()) as u16).to_le_bytes());
    bytes.extend_from_slice(&0x001f01ff_u32.to_le_bytes());
    bytes.extend_from_slice(user);
    descriptor(&bytes, user, false, false, true)?;
    Ok(bytes
        .chunks_exact(4)
        .map(|part| u32::from_le_bytes([part[0], part[1], part[2], part[3]]))
        .collect())
}

fn is_sid(bytes: &[u8], subauthorities: &[u32]) -> bool {
    bytes.len() == 8 + subauthorities.len() * 4
        && bytes[..8] == [1, subauthorities.len() as u8, 0, 0, 0, 0, 0, 5]
        && bytes[8..]
            .chunks_exact(4)
            .zip(subauthorities)
            .all(|(a, b)| a == b.to_le_bytes())
}

fn trusted(bytes: &[u8], user: &[u8], root: bool) -> bool {
    bytes == user
        || is_sid(bytes, &[18])
        || is_sid(bytes, &[32, 544])
        || (root
            && is_sid(
                bytes,
                &[
                    80, 956008885, 3418522649, 1831038044, 1853292631, 2271478464,
                ],
            ))
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, HostError> {
    let value = bytes.get(at..at + 2).ok_or(HostError::Insecure)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}
fn u32_at(bytes: &[u8], at: usize) -> Result<u32, HostError> {
    let value = bytes.get(at..at + 4).ok_or(HostError::Insecure)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}
fn offset(bytes: &[u8], at: usize) -> Result<usize, HostError> {
    let value = u32_at(bytes, at)? as usize;
    if value < 20 || !value.is_multiple_of(4) || value >= bytes.len() {
        return Err(HostError::Insecure);
    }
    Ok(value)
}

fn generic(mask: u32) -> u32 {
    let mut mapped = mask & 0x0fff_ffff;
    if mask & 0x8000_0000 != 0 {
        mapped |= 0x0012_0089;
    }
    if mask & 0x4000_0000 != 0 {
        mapped |= 0x0012_0116;
    }
    if mask & 0x2000_0000 != 0 {
        mapped |= 0x0012_00a0;
    }
    if mask & 0x1000_0000 != 0 {
        mapped |= 0x001f_01ff;
    }
    mapped
}

/// Conservative allow-union, not an AccessCheck clone: deny cannot rescue unsafe allows.
pub(super) fn descriptor(
    bytes: &[u8],
    user: &[u8],
    directory: bool,
    root: bool,
    secret: bool,
) -> Result<(), HostError> {
    if bytes.len() < 20
        || bytes.len() > MAX_DESCRIPTOR
        || bytes[..2] != [1, 0]
        || u16_at(bytes, 2)? & 0x8004 != 0x8004
        || u32_at(bytes, 12)? != 0
        || sid(user)? != user
    {
        return Err(HostError::Insecure);
    }
    let owner_at = offset(bytes, 4)?;
    let owner = sid(&bytes[owner_at..])?;
    if !trusted(owner, user, root) {
        return Err(HostError::Insecure);
    }
    let acl_at = offset(bytes, 16)?;
    let acl_size = usize::from(u16_at(bytes, acl_at + 2)?);
    let acl = bytes
        .get(acl_at..acl_at + acl_size)
        .ok_or(HostError::Insecure)?;
    if acl.len() < 8
        || !acl.len().is_multiple_of(4)
        || !matches!(acl[0], 2 | 4)
        || acl[1] != 0
        || acl[6..8] != [0, 0]
    {
        return Err(HostError::Insecure);
    }
    let overlaps_acl = |at: usize, size: usize| at < acl_at + acl_size && acl_at < at + size;
    if overlaps_acl(owner_at, owner.len()) {
        return Err(HostError::Insecure);
    }
    if u32_at(bytes, 8)? != 0 {
        let at = offset(bytes, 8)?;
        let group = sid(&bytes[at..])?;
        if overlaps_acl(at, group.len())
            || (at != owner_at && at < owner_at + owner.len() && owner_at < at + group.len())
        {
            return Err(HostError::Insecure);
        }
    }
    let count = usize::from(u16_at(acl, 4)?);
    if count > 4096 {
        return Err(HostError::Insecure);
    }
    let mut cursor = 8;
    for _ in 0..count {
        let size = usize::from(u16_at(acl, cursor + 2)?);
        let ace = acl.get(cursor..cursor + size).ok_or(HostError::Insecure)?;
        if size < 20 || !size.is_multiple_of(4) || !matches!(ace[0], 0 | 1) || ace[1] & !0x1f != 0 {
            return Err(HostError::Insecure);
        }
        let trustee = sid(&ace[8..])?;
        if trustee.len() + 8 != ace.len() {
            return Err(HostError::Insecure);
        }
        let mask = generic(u32_at(ace, 4)?);
        if mask & !0x001f_01ff != 0 {
            return Err(HostError::Insecure);
        }
        if ace[0] == 0 && ace[1] & 0x08 == 0 && !trusted(trustee, user, root) {
            let allowed = if secret && !directory {
                0x0012_0080
            } else {
                0x0012_00a9
            } | if root { 4 } else { 0 };
            if mask & !allowed != 0 {
                return Err(HostError::Insecure);
            }
        }
        cursor += size;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const USER: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 5, 42, 0, 0, 0];
    const WORLD: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    #[test]
    fn created_log_descriptor_has_one_exact_owner_ace_and_no_inheritance() {
        for count in 1..=15 {
            let mut user = vec![0_u8; 8 + count * 4];
            user[..8].copy_from_slice(&[1, count as u8, 0, 0, 0, 0, 0, 5]);
            user[8] = 42;
            let words = log_descriptor(&user).unwrap();
            let bytes: Vec<_> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
            assert_eq!(bytes.len(), 36 + user.len() * 2);
            assert_eq!(u16_at(&bytes, 2).unwrap(), 0x9004);
            assert_eq!(u32_at(&bytes, 4).unwrap(), 20);
            assert_eq!(u32_at(&bytes, 8).unwrap(), 0);
            assert_eq!(u32_at(&bytes, 12).unwrap(), 0);
            assert_eq!(&bytes[20..20 + user.len()], user);
            let acl = 20 + user.len();
            assert_eq!(u32_at(&bytes, 16).unwrap() as usize, acl);
            assert_eq!(u16_at(&bytes, acl + 4).unwrap(), 1);
            assert_eq!(&bytes[acl + 8..acl + 10], [0, 0]);
            assert_eq!(u32_at(&bytes, acl + 12).unwrap(), 0x001f01ff);
            assert_eq!(&bytes[acl + 16..], user);
            assert!(descriptor(&bytes, &user, false, false, true).is_ok());
            for length in 0..user.len() {
                assert!(log_descriptor(&user[..length]).is_err());
            }
            user.push(0);
            assert!(log_descriptor(&user).is_err());
        }
        for (offset, value) in [(0, 0), (0, 2), (1, 0), (1, 16), (1, 255)] {
            let mut invalid = USER;
            invalid[offset] = value;
            assert!(log_descriptor(&invalid).is_err());
        }
    }
    fn fixture(mask: u32, kind: u8, flags: u8) -> Vec<u8> {
        let mut bytes = vec![0; 60];
        bytes[0] = 1;
        bytes[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        bytes[4..8].copy_from_slice(&20u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&32u32.to_le_bytes());
        bytes[20..32].copy_from_slice(&USER);
        bytes[32..40].copy_from_slice(&[2, 0, 28, 0, 1, 0, 0, 0]);
        bytes[40..44].copy_from_slice(&[kind, flags, 20, 0]);
        bytes[44..48].copy_from_slice(&mask.to_le_bytes());
        bytes[48..].copy_from_slice(&WORLD);
        bytes
    }
    #[test]
    fn dacl_scope_inheritance_and_deny_reductions_are_conservative() {
        for mask in [1, 8, 0x20, 0x80000000, 0x20000000] {
            let bytes = fixture(mask, 0, 0x10);
            assert!(descriptor(&bytes, &USER, false, false, false).is_ok());
            assert_eq!(
                descriptor(&bytes, &USER, false, false, true),
                Err(HostError::Insecure)
            );
            assert!(descriptor(&fixture(mask, 0, 0x08), &USER, false, false, true).is_ok());
        }
        assert!(descriptor(&fixture(0x120080, 0, 0), &USER, false, false, true).is_ok());
        for mask in [
            2, 4, 16, 64, 0x100, 0x10000, 0x40000, 0x80000, 0x40000000, 0x10000000,
        ] {
            assert!(descriptor(&fixture(mask, 0, 0), &USER, false, false, false).is_err());
        }
        assert!(descriptor(&fixture(4, 0, 0), &USER, true, true, false).is_ok());
        assert!(descriptor(&fixture(4, 0, 0), &USER, true, false, false).is_err());
        assert!(descriptor(&fixture(0x1f01ff, 1, 0), &USER, false, false, true).is_ok());
        let mut bytes = fixture(1, 1, 0);
        bytes[34..36].copy_from_slice(&48u16.to_le_bytes());
        bytes[36] = 2;
        bytes.extend_from_slice(&fixture(1, 0, 0)[40..]);
        assert!(descriptor(&bytes, &USER, false, false, true).is_err());
    }
    #[test]
    fn malformed_offsets_sids_aces_and_descriptors_never_pass_or_panic() {
        let good = fixture(0x120080, 0, 0);
        assert!(descriptor(&good, &USER, false, false, true).is_ok());
        for length in 0..good.len() {
            assert!(descriptor(&good[..length], &USER, false, false, true).is_err());
        }
        for (offset, values) in [
            (0, vec![0, 2]),
            (1, vec![1]),
            (2, vec![0]),
            (3, vec![0]),
            (4, vec![0, 1, 21, 255]),
            (16, vec![0, 1, 20, 33, 255]),
            (20, vec![0, 2]),
            (21, vec![0, 16, 255]),
            (32, vec![0, 1, 3, 5]),
            (33, vec![1]),
            (34, vec![0, 1, 27, 255]),
            (36, vec![2, 255]),
            (40, vec![2, 5, 6, 9, 10, 17, 255]),
            (41, vec![32, 64, 128]),
            (42, vec![0, 19, 21, 255]),
            (48, vec![0, 2]),
            (49, vec![0, 16, 255]),
        ] {
            for value in values {
                let mut bytes = good.clone();
                bytes[offset] = value;
                assert!(
                    descriptor(&bytes, &USER, false, false, true).is_err(),
                    "offset {offset} value {value}"
                );
            }
        }
        assert!(descriptor(&vec![0; MAX_DESCRIPTOR + 1], &USER, false, false, true).is_err());
        let mut foreign = good;
        foreign[28] = 43;
        assert!(descriptor(&foreign, &USER, false, false, true).is_err());
    }
    #[test]
    fn path_and_sid_resource_bounds_are_checked() {
        assert!(path("C:\\키 저장소\\identity.age").is_ok());
        assert!(path(&format!("C:\\{}", "a".repeat(256))).is_err());
        assert!(path(&format!("C:\\{}file", "a\\".repeat(128))).is_err());
        assert!(path(&format!("C:\\{}file", "a".repeat(MAX_PATH))).is_err());
        for length in 0..12 {
            assert!(sid(&USER[..length]).is_err());
        }
        assert_eq!(sid(&USER).unwrap(), USER);
        assert!(
            same_path(
                "root\\Long Directory\\identity.key",
                "ROOT\\long directory\\IDENTITY.KEY"
            )
            .is_ok()
        );
        for observed in [
            "root\\LONGDI~1\\identity.key",
            "other\\Long Directory\\identity.key",
            "root\\Long Directory\\other.key",
        ] {
            assert!(same_path("root\\Long Directory\\identity.key", observed).is_err());
        }
        assert!(same_path("root\\Ä.key", "root\\ä.key").is_err());
        assert!(volume_mapping("\\Device\\HarddiskVolume12").is_ok());
        for mapping in [
            "\\??\\C:\\",
            "\\??\\C:\\folder",
            "\\Device\\LanmanRedirector",
            "\\Device\\HarddiskVolume1\\sub",
            "\\Device\\HarddiskVolume",
            "\\Device\\HarddiskVolume12345678901",
        ] {
            assert!(volume_mapping(mapping).is_err());
        }
    }
}
