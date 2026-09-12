//! Bound newline-delimited MCP frames before the SDK allocates/parses a whole message.
use std::{
    io::{Error, ErrorKind, Result as IoResult},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

pub const MAX_FRAME_BYTES: usize = 65_536;

pub struct BoundedInput<R> {
    inner: R,
    line_bytes: usize,
    failed: bool,
}

impl<R> BoundedInput<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            line_bytes: 0,
            failed: false,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for BoundedInput<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        destination: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        let this = self.get_mut();
        if this.failed {
            return Poll::Ready(Err(Error::new(ErrorKind::InvalidData, "frame_too_large")));
        }
        if destination.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let mut scratch = [0_u8; 4096];
        let capacity = destination.remaining().min(scratch.len());
        let mut read = ReadBuf::new(&mut scratch[..capacity]);
        match Pin::new(&mut this.inner).poll_read(cx, &mut read) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {
                for byte in read.filled() {
                    if *byte == b'\n' {
                        this.line_bytes = 0;
                    } else {
                        this.line_bytes += 1;
                        if this.line_bytes > MAX_FRAME_BYTES {
                            this.failed = true;
                            return Poll::Ready(Err(Error::new(
                                ErrorKind::InvalidData,
                                "frame_too_large",
                            )));
                        }
                    }
                }
                destination.put_slice(read.filled());
                Poll::Ready(Ok(()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn rejects_oversize_unterminated_frame_before_full_allocation() {
        let bytes = vec![b'x'; MAX_FRAME_BYTES * 16];
        let mut input = BoundedInput::new(bytes.as_slice());
        let mut accepted = Vec::new();
        assert!(input.read_to_end(&mut accepted).await.is_err());
        assert!(accepted.len() <= MAX_FRAME_BYTES);
        assert!(input.read_u8().await.is_err());
    }

    #[tokio::test]
    async fn bounds_each_line_not_session_total() {
        let bytes = vec![b'\n'; MAX_FRAME_BYTES * 2];
        let mut input = BoundedInput::new(bytes.as_slice());
        let mut accepted = Vec::new();
        input.read_to_end(&mut accepted).await.unwrap();
        assert_eq!(accepted, bytes);
    }
}
