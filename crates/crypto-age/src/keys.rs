use crate::stream::Budget;
use openwrt_mcp_runtime::protection::ProtectionError;

const MAX_NATIVE_KEY_LINE_BYTES: usize = 128;

/// Deliberately accept trimmed native keys, empty lines and whole-line comments.
/// No IdentityFile parser is used: it could admit extra identity schemes.
fn lines(bytes: &[u8]) -> Result<impl Iterator<Item = &str>, ProtectionError> {
    let document = std::str::from_utf8(bytes).map_err(|_| ProtectionError::InvalidMaterial)?;
    Ok(document
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#')))
}

pub(crate) fn recipients(
    bytes: &[u8],
    maximum: usize,
    budget: &Budget,
) -> Result<Vec<age::x25519::Recipient>, ProtectionError> {
    let mut recipients = Vec::new();
    for line in lines(bytes)? {
        budget.check()?;
        if recipients.len() >= maximum {
            return Err(ProtectionError::ResourceLimit);
        }
        if line.len() > MAX_NATIVE_KEY_LINE_BYTES {
            return Err(ProtectionError::InvalidMaterial);
        }
        recipients.push(line.parse().map_err(|_| ProtectionError::InvalidMaterial)?);
    }
    if recipients.is_empty() {
        return Err(ProtectionError::InvalidMaterial);
    }
    budget.check()?;
    Ok(recipients)
}

pub(crate) fn identities(
    bytes: &[u8],
    maximum: usize,
    budget: &Budget,
) -> Result<Vec<age::x25519::Identity>, ProtectionError> {
    let mut identities = Vec::new();
    for line in lines(bytes)? {
        budget.check()?;
        if identities.len() >= maximum {
            return Err(ProtectionError::ResourceLimit);
        }
        if line.len() > MAX_NATIVE_KEY_LINE_BYTES {
            return Err(ProtectionError::InvalidMaterial);
        }
        // age parses only native X25519 and zeroizes decoded secret intermediates.
        identities.push(line.parse().map_err(|_| ProtectionError::InvalidMaterial)?);
    }
    if identities.is_empty() {
        return Err(ProtectionError::InvalidMaterial);
    }
    budget.check()?;
    Ok(identities)
}
