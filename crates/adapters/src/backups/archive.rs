use openwrt_mcp_device_codec::{
    archive::{ArchiveValidator, ExpectedArchive, ExpectedFile},
    gzip::GzipArchiveValidator,
};
use openwrt_mcp_runtime::{
    backups::{ArchiveFormat, ArtifactBinding, BackupError},
    mutation_ports::WorkBudget,
    sealing::{CheckCounts, SealCheck, SealPortError},
};

enum Validator {
    Tar(Box<ArchiveValidator>),
    Gzip(Box<GzipArchiveValidator>),
}
pub(super) struct Check {
    validator: Validator,
    budget: WorkBudget,
}
impl Check {
    pub(super) fn new(binding: &ArtifactBinding, budget: WorkBudget) -> Result<Self, BackupError> {
        budget.check()?;
        let file = ExpectedFile::new("etc/config/system", binding.file_bytes())
            .map_err(|_| BackupError::Invalid)?;
        let expected = ExpectedArchive::new(&[file]).map_err(|_| BackupError::Invalid)?;
        let validator = match binding.format() {
            ArchiveFormat::Tar => Validator::Tar(Box::new(ArchiveValidator::new(expected))),
            ArchiveFormat::Gzip => Validator::Gzip(Box::new(GzipArchiveValidator::new(expected))),
        };
        Ok(Self { validator, budget })
    }
}
impl SealCheck for Check {
    fn feed(&mut self, bytes: &[u8]) -> Result<(), SealPortError> {
        self.budget.check().map_err(|_| SealPortError)?;
        match &mut self.validator {
            Validator::Tar(v) => v.feed(bytes).map_err(|_| SealPortError)?,
            Validator::Gzip(v) => v.feed(bytes).map_err(|_| SealPortError)?,
        };
        self.budget.check().map_err(|_| SealPortError)
    }
    fn finish(self: Box<Self>) -> Result<CheckCounts, SealPortError> {
        self.budget.check().map_err(|_| SealPortError)?;
        let counts = match self.validator {
            Validator::Tar(v) => {
                let s = v.finish().map_err(|_| SealPortError)?;
                CheckCounts {
                    source_bytes: s.archive_bytes,
                    expanded_bytes: s.archive_bytes,
                    payload_bytes: s.payload_bytes,
                    files: s.files as u64,
                }
            }
            Validator::Gzip(v) => {
                let s = v.finish().map_err(|_| SealPortError)?;
                CheckCounts {
                    source_bytes: s.compressed_bytes,
                    expanded_bytes: s.archive_bytes,
                    payload_bytes: s.payload_bytes,
                    files: s.files as u64,
                }
            }
        };
        self.budget.check().map_err(|_| SealPortError)?;
        Ok(counts)
    }
}
