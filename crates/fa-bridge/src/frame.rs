use std::io::Write;

use crate::error::{BridgeError, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn read_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<String> {
    let mut len_buf = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).await?;
        if byte[0] == b' ' {
            break;
        }
        len_buf.push(byte[0]);
    }

    let len_str = String::from_utf8(len_buf)
        .map_err(|e| BridgeError::InvalidProtocol(format!("invalid length: {e}")))?;
    let length: usize = len_str
        .parse()
        .map_err(|e| BridgeError::InvalidProtocol(format!("invalid length: {e}")))?;

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).await?;

    let mut trailing = [0u8; 1];
    match reader.read(&mut trailing).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {}
        Err(e) => return Err(e.into()),
    }

    String::from_utf8(body).map_err(|e| BridgeError::InvalidProtocol(format!("invalid utf8: {e}")))
}

pub async fn write_frame<W: AsyncWriteExt + Unpin>(writer: &mut W, message: &str) -> Result<()> {
    let mut buf = Vec::new();
    write!(buf, "{} ", message.len()).unwrap();
    buf.extend_from_slice(message.as_bytes());
    buf.push(b'\n');
    writer.write_all(&buf).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn write_and_read_frame() {
        let message = "hello world";
        let mut buf = Vec::new();
        write_frame(&mut buf, message).await.unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_frame(&mut cursor).await.unwrap();
        assert_eq!(result, message);
    }

    #[tokio::test]
    async fn write_frame_format() {
        let mut buf = Vec::new();
        write_frame(&mut buf, "abc").await.unwrap();
        assert_eq!(buf, b"3 abc\n".to_vec());
    }
}
