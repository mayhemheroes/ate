#[allow(unused_imports)]
use tracing::{info, warn, debug, error, trace, instrument, span, Level};
use serde::*;

use super::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sudo {
    pub email: String,
    pub uid: u32,
    pub google_auth: String,
    pub secret: String,
    pub qr_code: String,
    pub failed_attempts: u32,
    pub access: Vec<Authorization>,
    pub groups: Vec<String>,
}