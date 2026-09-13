//! Development-only stdin check for two public synthetic files, not a backup CLI.
//! Expected a = "public\n" (7 bytes), empty = empty, both under fixture/.
use openwrt_mcp_device_codec::archive::{
    ArchiveValidator, ExpectedArchive, ExpectedFile, MAX_CHUNK_BYTES,
};
use std::io::Read;

fn check() -> Result<(), &'static str> {
    let files = [
        ExpectedFile::new("fixture/a", 7).map_err(|_| "fixture_manifest_failed")?,
        ExpectedFile::new("fixture/empty", 0).map_err(|_| "fixture_manifest_failed")?,
    ];
    let expected = ExpectedArchive::new(&files).map_err(|_| "fixture_manifest_failed")?;
    let mut validator = ArchiveValidator::new(expected);
    let mut chunk = [0; MAX_CHUNK_BYTES];
    let mut input = std::io::stdin().lock();
    loop {
        let count = input.read(&mut chunk).map_err(|_| "fixture_input_failed")?;
        if count == 0 {
            break;
        }
        validator.feed(&chunk[..count]).map_err(|e| e.code())?;
    }
    let result = validator.finish().map_err(|e| e.code())?;
    println!(
        "files={} payload_bytes={} archive_bytes={}",
        result.files, result.payload_bytes, result.archive_bytes
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match check() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{code}");
            std::process::ExitCode::FAILURE
        }
    }
}
