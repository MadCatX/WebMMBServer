use std::convert::TryInto;

use crate::server::api;

impl api::TransferChunk {
    pub fn from_bytes(v: &[u8]) -> Result<api::TransferChunk, String> {
        if v.len() < 77 {
            return Err(String::from("Array must be at least 77 bytes long"));
        }

        let job_id = match String::from_utf8(Vec::from(&v[0..36])) {
            Ok(id) => id,
            Err(_) => return Err(String::from("job_id part is not a valid UTF-8 byte sequence")),
        };
        let transfer_id = match String::from_utf8(Vec::from(&v[36..72])) {
            Ok(id) => id,
            Err(_) => return Err(String::from("transfer_id part is not a valid UTF-8 byte sequence")),
        };
        let challenge = i32::from_le_bytes(v[72..76].try_into().unwrap());

        let data = Vec::from(&v[76..]);

        Ok(api::TransferChunk{job_id, transfer_id, challenge, data})
    }
}
