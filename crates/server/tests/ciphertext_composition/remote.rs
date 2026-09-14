use super::*;
use openwrt_mcp_adapters::backups::BackupWorker;
use openwrt_mcp_backend_ssh::{SshBackend, SshOptions};
use openwrt_mcp_core::{ProbeRequest, ReviewedObject};
use openwrt_mcp_runtime::{Backend, Limits};
use russh::{Channel, ChannelId, server};
use tokio::net::TcpListener;

fn ssh_key(seed: u8) -> russh::keys::PrivateKey {
    russh::keys::PrivateKey::new(
        russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[seed; 32]).into(),
        "synthetic-p3",
    )
    .unwrap()
}
struct CaptureServer {
    channels: Vec<Channel<server::Msg>>,
    expected: Vec<u8>,
    request: Vec<u8>,
    payload: Vec<u8>,
    captures: Arc<AtomicU64>,
}
impl server::Handler for CaptureServer {
    type Error = russh::Error;
    async fn auth_publickey(
        &mut self,
        user: &str,
        key: &russh::keys::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(
            if user == "fixture" && key.key_data() == ssh_key(62).public_key().key_data() {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        self.channels.push(channel);
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        _: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        session.data(id, b"'system' @00000001\n\t\"info\":{}\n".to_vec())?;
        session.exit_status_request(id, 0)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(name, "openwrt-mcp-capture-v1");
        session.channel_success(id)?;
        Ok(())
    }
    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.request.extend_from_slice(data);
        if self.request.len() == self.expected.len() {
            assert!(self.request == self.expected);
            self.captures.fetch_add(1, Ordering::SeqCst);
            for chunk in self
                .expected
                .iter()
                .chain(self.payload.iter())
                .copied()
                .collect::<Vec<_>>()
                .chunks(997)
            {
                session.data(id, chunk.to_vec())?;
            }
            session.exit_status_request(id, 0)?;
            session.eof(id)?;
            session.close(id)?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn pinned_ssh_capture_to_owned_worker_native_store_age_authentication_and_inspection() {
    let data = tar(b"synthetic");
    let bound = binding(9, data.len(), ArchiveFormat::Tar, 4);
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let host = ssh_key(61);
    let options = SshOptions {
        host: "127.0.0.1".into(),
        port: listener.local_addr().unwrap().port(),
        username: "fixture".into(),
        host_key_sha256: host
            .public_key()
            .fingerprint(russh::keys::HashAlg::Sha256)
            .to_string(),
    };
    let captures = Arc::new(AtomicU64::new(0));
    let observed = captures.clone();
    let expected = bound.encoded().to_vec();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let config = Arc::new(server::Config {
            keys: vec![host],
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let handler = CaptureServer {
            channels: vec![],
            expected,
            request: vec![],
            payload: data,
            captures: observed,
        };
        let running = server::run_stream(config, stream, handler).await.unwrap();
        let _ = running.await;
    });
    let key = ssh_key(62)
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .unwrap();
    let backend = SshBackend::new_for_capture(
        options,
        Arc::new(Keys(
            KeyMaterial::new(key.as_bytes().to_vec(), 65536).unwrap(),
        )),
        [2; 16],
    )
    .unwrap();
    assert!(
        backend
            .capture_archive(bound.clone(), budget())
            .await
            .is_err()
    );
    backend
        .probe(
            ProbeRequest::DescribeUbusObject(ReviewedObject::System),
            &Limits::default(),
        )
        .await
        .unwrap();
    let work = budget();
    let captured = backend
        .capture_archive(bound.clone(), work.clone())
        .await
        .unwrap();
    assert_eq!(captures.load(Ordering::SeqCst), 1);
    let crypto = Crypto::new();
    let file = StoreFile::new(crypto.auth.as_ref());
    let path = file.path.clone();
    let expected = bound.clone();
    let worker = BackupWorker::spawn(work, move |budget| {
        let mut store = RecordStore::open(&path, [1; 16], crypto.auth.clone(), &budget)?;
        let sealed = seal_capture(captured, &crypto.encrypt, &mut store, budget.clone())
            .map_err(|_| BackupError::Unknown)?;
        let inspected = restore_inspect(&mut store, &expected, &crypto.decrypt, budget)?;
        assert_eq!(sealed, inspected);
        Ok(inspected)
    })
    .unwrap();
    // Never synchronously join while a worker depends on this executor. This one
    // owns only bounded synchronous crypto/file work after asynchronous capture.
    while !worker.is_finished() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let result = worker.join().unwrap();
    assert_eq!(result.payload_bytes, 9);
    drop(backend);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fs::read_dir(&file.root).unwrap().count(), 1);
}
