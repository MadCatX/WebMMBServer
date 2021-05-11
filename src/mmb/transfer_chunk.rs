use crate::server::api;

impl api::TransferChunk {
    pub fn from_bytes(v: &[u8]) -> Result<api::TransferChunk, String> {
        if v.len() < 73 {
            return Err(String::from("Array must be at least 73 bytes long"));
        }

        let job_id = match String::from_utf8(Vec::from(&v[0..36])) {
            Ok(id) => id,
            Err(_) => return Err(String::from("job_id part is not a valid UTF-8 byte sequence")),
        };
        let transfer_id = match String::from_utf8(Vec::from(&v[36..72])) {
            Ok(id) => id,
            Err(_) => return Err(String::from("transfer_id part is not a valid UTF-8 byte sequence")),
        };

        let data = Vec::from(&v[72..]);

        Ok(api::TransferChunk{job_id, transfer_id, data})
    }
}
