use anyhow::{Result, anyhow};
use bytes::{Buf, BytesMut};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

pub fn write_var_int(vec: &mut Vec<u8>, num: i32) {
    let mut value = num;
    loop {
        if value & 0xFFFFFF80u32 as i32 != 0 {
            vec.push((value & 0x7F | 0x80) as u8);
            value = value >> 7 & 0x1FFFFFF;
        } else {
            vec.push((value & 0x7F) as u8);
            break;
        }
    }
}

pub async fn read_var_int_stream(stream: &mut TcpStream) -> Result<i32> {
    let mut result: i32 = 0;
    let mut offset = 0;
    loop {
        let num = stream.read_u8().await?;
        if offset > 5 {
            return Err(anyhow!("Invalid varint: Too long"));
        }
        result |= i32::from(num & 0x7F) << (offset * 7);
        offset += 1;
        if num & 0x80 == 0 {
            break;
        }
    }
    Ok(result)
}

pub fn read_var_int_buf(buf: &mut BytesMut) -> Result<i32> {
    let mut result: i32 = 0;
    let mut offset = 0;
    loop {
        let num = buf.get_u8();
        if offset > 5 {
            return Err(anyhow!("Invalid varint: Too long"));
        }
        result |= i32::from(num & 0x7F) << (offset * 7);
        offset += 1;
        if num & 0x80 == 0 {
            break;
        }
    }
    Ok(result)
}

pub fn read_string(buf: &mut BytesMut) -> Result<String> {
    let length = read_var_int_buf(buf)? as usize;
    if buf.remaining() < length {
        return Err(anyhow!("Unexpected end of string"));
    }
    let str = String::from_utf8(buf.split_to(length).to_vec());
    match str {
        Ok(s) => Ok(s),
        Err(e) => Err(anyhow!(e)),
    }
}
