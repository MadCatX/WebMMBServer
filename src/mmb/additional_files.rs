use crate::mmb;

const RESERVED_FILE_NAMES: &'static[&'static str] = &[
    mmb::CMDS_FILE_NAME,
    mmb::DOUT_FILE_NAME,
    mmb::PGRS_FILE_NAME,
    mmb::PARAMS_FILE_NAME,
    "frame.pdb",
];

pub fn is_reserved_file_name(name: &String) -> bool {
    let lwr = name.to_lowercase();

    for s in RESERVED_FILE_NAMES.iter() {
        if *s == lwr.as_str() {
            return true;
        }
    }

    if name.starts_with(mmb::TRAJECTORY_FILE_PREFIX) {
        return true;
    }
    if name.starts_with(mmb::LAST_FRAME_FILE_PREFIX) {
        return true;
    }

    false
}
