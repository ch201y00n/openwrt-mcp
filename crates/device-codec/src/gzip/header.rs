use super::{GzipError, MAX_COMMENT_BYTES, MAX_EXTRA_BYTES, MAX_HEADER_BYTES, MAX_NAME_BYTES};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
enum Stage {
    Fixed,
    ExtraLength { low: Option<u8> },
    Extra { remaining: usize },
    Name { bytes: usize },
    Comment { bytes: usize },
    Crc { remaining: usize },
    Done,
}

/// Linear admission before the original header reaches the integrity backend.
pub(super) struct Header {
    bytes: Zeroizing<[u8; MAX_HEADER_BYTES]>,
    length: usize,
    flags: u8,
    stage: Stage,
}
impl Header {
    pub(super) fn new() -> Self {
        Self {
            bytes: Zeroizing::new([0; MAX_HEADER_BYTES]),
            length: 0,
            flags: 0,
            stage: Stage::Fixed,
        }
    }
    pub(super) fn complete(&self) -> bool {
        matches!(self.stage, Stage::Done)
    }
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }

    pub(super) fn feed(&mut self, input: &[u8]) -> Result<usize, GzipError> {
        let mut consumed = 0;
        for &byte in input {
            if self.complete() {
                break;
            }
            if self.length == MAX_HEADER_BYTES {
                return Err(GzipError::LimitExceeded);
            }
            self.bytes[self.length] = byte;
            self.length += 1;
            consumed += 1;
            match self.stage {
                Stage::Fixed if self.length == 10 => {
                    if self.bytes[..3] != [0x1f, 0x8b, 8] || self.bytes[3] & 0xe0 != 0 {
                        return Err(GzipError::InvalidHeader);
                    }
                    self.flags = self.bytes[3];
                    self.advance();
                }
                Stage::Fixed => {}
                Stage::ExtraLength { low: None } => {
                    self.stage = Stage::ExtraLength { low: Some(byte) }
                }
                Stage::ExtraLength { low: Some(low) } => {
                    let remaining = usize::from(u16::from_le_bytes([low, byte]));
                    if remaining > MAX_EXTRA_BYTES {
                        return Err(GzipError::LimitExceeded);
                    }
                    if remaining == 0 {
                        self.advance();
                    } else {
                        self.stage = Stage::Extra { remaining };
                    }
                }
                Stage::Extra { remaining: 1 } | Stage::Crc { remaining: 1 } => self.advance(),
                Stage::Extra { remaining } => {
                    self.stage = Stage::Extra {
                        remaining: remaining - 1,
                    }
                }
                Stage::Crc { remaining } => {
                    self.stage = Stage::Crc {
                        remaining: remaining - 1,
                    }
                }
                Stage::Name { bytes } => {
                    let bytes = bytes + 1;
                    if bytes > MAX_NAME_BYTES {
                        return Err(GzipError::LimitExceeded);
                    }
                    if byte == 0 {
                        self.advance();
                    } else {
                        self.stage = Stage::Name { bytes };
                    }
                }
                Stage::Comment { bytes } => {
                    let bytes = bytes + 1;
                    if bytes > MAX_COMMENT_BYTES {
                        return Err(GzipError::LimitExceeded);
                    }
                    if byte == 0 {
                        self.advance();
                    } else {
                        self.stage = Stage::Comment { bytes };
                    }
                }
                Stage::Done => return Err(GzipError::InvalidHeader),
            }
        }
        Ok(consumed)
    }

    fn advance(&mut self) {
        self.stage = if self.flags & 4 != 0 {
            self.flags &= !4;
            Stage::ExtraLength { low: None }
        } else if self.flags & 8 != 0 {
            self.flags &= !8;
            Stage::Name { bytes: 0 }
        } else if self.flags & 16 != 0 {
            self.flags &= !16;
            Stage::Comment { bytes: 0 }
        } else if self.flags & 2 != 0 {
            self.flags &= !2;
            Stage::Crc { remaining: 2 }
        } else {
            Stage::Done
        };
    }
}
