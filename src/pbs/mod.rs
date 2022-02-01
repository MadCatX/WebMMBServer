use std::process::Command;
use serde_json;

pub enum JobState {
    Queued,
    Running,
    Exiting,
    Finished,
    Held,
    Unknown,
}

pub struct JobInfo {
    pub state: JobState,
    pub exec_node: String,
}

pub struct ServerInfo {
    pub version: VersionInfo,
    pub name: String,
}

pub struct VersionInfo {
    pub major: i32,
    pub minor: i32,
    pub revision: i32,
}

type JsonObject = serde_json::Map<String, serde_json::Value>;

fn get_pbs_state_json() -> Result<JsonObject, String> {
    match Command::new("qstat")
                  .args(&["-f", "-F", "json"])
                  .output() {
        Ok(output) => {
            if !output.status.success() {
                return Err(String::from("Failed to execute qstat"));
            }

            match String::from_utf8(output.stdout) {
                Ok(raw) => match serde_json::from_str::<JsonObject>(raw.as_str()) {
                    Ok(obj) => Ok(obj),
                    Err(e) => Err(e.to_string()),
                },
                Err(_) => return Err(String::from("qstat output is not a valid string")),
            }
        },
        Err(e) => Err(e.to_string()),
    }
}

fn parse_server_info(pbs_state: &JsonObject) -> Result<ServerInfo, String> {
    let mut info = ServerInfo{
        name: String::new(),
        version: VersionInfo{major: 0, minor: 0, revision: 0},
    };

    info.name = match pbs_state.get("pbs_server") {
        Some(v) => match serde_json::from_value::<String>(v.clone()) {
            Ok(name) => name,
            Err(e) => return Err(format!("Cannot get server name from PBS status object: {}", e)),
        },
        None => return Err(String::from("pbs_server field not found")),
    };
    info.version = match pbs_state.get("pbs_version") {
        Some(v) => match serde_json::from_value::<String>(v.clone()) {
            Ok(ver_str) => {
                let segments: Vec<&str> = ver_str.split(".").collect();
                if segments.len() != 3 {
                    return Err(String::from("Invalid version string"));
                }
                let major = match segments[0].parse::<i32>() {
                    Ok(n) => n,
                    Err(_) => return Err(String::from("Invalid major version info!")),
                };
                let minor = match segments[1].parse::<i32>() {
                    Ok(n) => n,
                    Err(_) => return Err(String::from("Invalid minor version info!")),
                };
                let revision = match segments[2].parse::<i32>() {
                    Ok(n) => n,
                    Err(_) => return Err(String::from("Invalid revision version info!")),
                };
                VersionInfo{major, minor, revision}
            },
            Err(e) => return Err(format!("Cannot get version info from PBS status object: {}", e)),
        },
        None => return Err(String::from("pbs_version field not found")),
    };

    Ok(info)
}

fn parse_job_info(job_no: u32, server_name: &String, pbs_state: &JsonObject) -> Result<JobInfo, String> {
    let mut info = JobInfo{
        state: JobState::Unknown,
        exec_node: String::new(),
    };

    let jobs = pbs_state.get("Jobs");
    if jobs.is_none() {
        return Ok(info);
    }

    let expected_job_name = format!("{}.{}", job_no, server_name);
    let job_obj = match jobs.unwrap().get(expected_job_name) {
        Some(v) => v,
        None => return Ok(info),
    };

    info.state = match job_obj.get("job_state") {
        Some(v) => match serde_json::from_value::<String>(v.clone()) {
            Ok(s) => match s.as_str() {
                "Q" => JobState::Queued,
                "R" => JobState::Running,
                "E" => JobState::Exiting,
                "F" => JobState::Finished,
                "H" => JobState::Held,
                _ => return Err(format!("Unknown job state {}", v)),
            },
            Err(e) => return Err(e.to_string()),
        },
        None => return Err(String::from("job_state field not found")),
    };
    info.exec_node = match job_obj.get("exec_host") {
        Some(v) => match serde_json::from_value::<String>(v.clone()) {
            Ok(s) => {
                let parts: Vec<&str> = s.split('/').collect();
                if parts.len() != 2 {
                    return Err(format!("String {} does not represent a valid exec_node ID", s));
                }
                String::from(parts[0])
            }
            Err(e) => return Err(format!("Cannot parse exec_node value: {}", e)),
        },
        None => return Err(String::from("exec_host field not found")),
    };

    Ok(info)
}

pub fn get_job_info(job_no: u32) -> Result<JobInfo, String> {
    match get_pbs_state_json() {
        Ok(state) => match parse_server_info(&state) {
            Ok(server_info) => parse_job_info(job_no, &server_info.name, &state),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}
