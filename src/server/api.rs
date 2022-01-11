use std::collections::HashMap;
use rocket::http::Status;
use serde_derive::{Deserialize, Serialize};
use serde_json;

/* JSON commands */

#[derive(Deserialize, Serialize, Clone)]
pub enum BondMobility {
    Rigid,
    Torsion,
    Free,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum CompoundType {
    DNA,
    Protein,
    RNA,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum EdgeInteraction {
    WatsonCrick,
    SugarEdge,
}

#[derive(Deserialize)]
pub enum FileOperationRequestType {
    InitUpload,
    FinishUpload,
    CancelUpload,
    Delete,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Orientation {
    Cis,
    Trans,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct BaseInteraction {
    pub chain_name_1: String,
    pub res_no_1: i32,
    pub edge_1: EdgeInteraction,
    pub chain_name_2: String,
    pub res_no_2: i32,
    pub edge_2: EdgeInteraction,
    pub orientation: Orientation,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Chain {
    pub name: String,
    pub auth_name: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Compound {
    pub chain: Chain,
    pub ctype: CompoundType,
    pub sequence: String,
    pub residues: Vec<ResidueNumber>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DoubleHelix {
    pub chain_name_1: String,
    pub first_res_no_1: i32,
    pub last_res_no_1: i32,
    pub chain_name_2: String,
    pub first_res_no_2: i32,
    pub last_res_no_2: i32,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Mobilizer {
    pub bond_mobility: BondMobility,
    pub chain: Option<String>,
    pub first_residue: Option<i32>,
    pub last_residue: Option<i32>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct NtCConformation {
    pub chain_name: String,
    pub first_res_no: i32,
    pub last_res_no: i32,
    pub ntc: String,
    pub weight: f64,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct NtCs {
    pub conformations: Vec<NtCConformation>,
    pub force_scale_factor: f64,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ResidueNumber {
    pub number: i32,
    pub auth_number: i32,
}

pub type JsonAdvancedParameters = HashMap<String, serde_json::Value>;

#[derive(Deserialize, Serialize, Clone)]
pub struct DensityFitCommands {
    /* Concrete commands */
    pub structure_file_name: String,
    pub density_map_file_name: String,
    pub compounds: Vec<Compound>,
    pub mobilizers: Vec<Mobilizer>,
    pub ntcs: NtCs,
    pub set_default_MD_parameters: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StandardCommands {
    /* Concrete commands */
    pub compounds: Vec<Compound>,
    pub double_helices: Vec<DoubleHelix>,
    pub base_interactions: Vec<BaseInteraction>,
    pub ntcs: NtCs,
    pub mobilizers: Vec<Mobilizer>,
    pub adv_params: JsonAdvancedParameters,
    pub set_default_MD_parameters: bool,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "job_type")]
pub enum ConcreteCommands {
    DensityFit(DensityFitCommands),
    Standard(StandardCommands),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Commands {
    pub first_stage: i32,
    pub last_stage: i32,
    pub reporting_interval: f64,
    pub num_reporting_intervals: i32,
    pub base_interaction_scale_factor: i32,
    pub temperature: f64,

    #[serde(flatten)]
    pub concrete: ConcreteCommands,
}

/* Requests */

#[derive(Deserialize)]
pub struct ApiRequestData {
    pub data: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(tag = "req_type")]
pub enum ApiRequest {
    StartJob(ApiRequestData),
    StartJobRaw(ApiRequestData),
    StopJob(ApiRequestData),
    CreateJob(ApiRequestData),
    DeleteJob(ApiRequestData),
    JobStatus(ApiRequestData),
    ListJobs(ApiRequestData),
    JobCommands(ApiRequestData),
    JobCommandsRaw(ApiRequestData),
    SessionInfo(ApiRequestData),
    MmbOutput(ApiRequestData),
    CloneJob(ApiRequestData),
    ListExamples(ApiRequestData),
    ActivateExample(ApiRequestData),
    FileOperation(ApiRequestData),
    ListAdditionalFiles(ApiRequestData),
}

#[derive(Deserialize)]
pub struct AuthRequestData {
    pub session_id: String,
}

#[derive(Deserialize)]
#[serde(tag = "auth_type")]
pub enum AuthRequest {
    LogIn(AuthRequestData),
    LogOut(AuthRequestData),
}

pub struct FileTransferChunk {
    pub job_id: String,
    pub transfer_id: String,
    pub index: u32,
    pub data: Vec<u8>,
}

#[derive(Deserialize)]
pub struct CloneJobRqData {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateJobRqData {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SimpleJobRqData {
    pub id: String,
}

#[derive(Deserialize)]
pub struct ResumeJobRqData {
    pub id: String,
    pub commands: Commands,
}
#[derive(Deserialize)]
pub struct StartJobRqData {
    pub id: String,
    pub commands: Commands,
}
#[derive(Deserialize)]
pub struct StartJobRawRqData {
    pub id: String,
    pub commands: String,
}
#[derive(Deserialize)]
pub struct FileOperationRqData {
    pub req_type: FileOperationRequestType,
    pub job_id: String,
    pub transfer_id: String,
    pub file_name: String,
}

/* Responses */

pub struct ApiResponse {
    pub is_ok: bool,
    pub ok_data: Option<serde_json::Value>,
    pub fail_data: Option<(Status, String)>,
}

pub struct AuthResponse {
    pub status: Status,
    pub message: String,
}

#[derive(Serialize)]
pub enum JobState {
    NotStarted,
    Queued,
    Running,
    Finished,
    Failed,
}

#[derive(Serialize)]
pub enum JobCommandsMode {
    None,
    Synthetic,
    Raw,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct OkResponse {
    pub success: bool,
    pub data: serde_json::Value,
}

#[derive(Serialize)]
pub struct AdditionalFile {
    pub name: String,
    pub size: String,
}

#[derive(Serialize)]
pub struct Empty {
}

#[derive(Serialize)]
pub struct ExampleListItem {
    pub name: String,
    pub description: String,
}

pub type ExampleList = Vec<ExampleListItem>;

#[derive(Serialize)]
pub struct FileTransferAck {
    pub id: String,
}

#[derive(Serialize)]
pub struct JobCommands {
    pub is_empty: bool,
    pub commands: Option<Commands>,
}

#[derive(Serialize)]
pub struct JobCommandsRaw {
    pub is_empty: bool,
    pub commands: Option<String>,
}

#[derive(Serialize)]
pub struct JobCreated {
    pub id: String,
}

#[derive(Serialize)]
pub struct JobInfo {
    pub id: String,
    pub name: String,
    pub created_on: String,
    pub commands_mode: JobCommandsMode,
    pub available_stages: Vec<i32>,
    pub current_stage: Option<i32>,
    pub state: JobState,
    pub progress: Option<JobProgress>,
}

#[derive(Serialize)]
pub struct JobProgress {
    pub step: String,
    pub total_steps: i32,
}

pub type JobList = Vec<JobInfo>;

#[derive(Serialize)]
pub struct SessionInfo {
    pub id: String,
}
