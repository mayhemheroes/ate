use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::watch;
#[allow(unused_imports, dead_code)]
use tracing::{debug, error, info, trace, warn};

use crate::common::*;
use crate::err::*;

#[derive(Debug)]
pub struct Process {
    pub(crate) pid: Pid,
    pub(crate) exit_rx: watch::Receiver<Option<i32>>,
    pub(crate) exit_tx: Arc<watch::Sender<Option<i32>>>,
}

impl Clone for Process {
    fn clone(&self) -> Process {
        Process {
            pid: self.pid,
            exit_rx: self.exit_rx.clone(),
            exit_tx: self.exit_tx.clone(),
        }
    }
}

impl Process {
    pub async fn wait_for_exit(&mut self) -> i32 {
        let mut ret = self.exit_rx.borrow().clone();
        while ret.is_none() {
            let state = self.exit_rx.changed().await;
            ret = self.exit_rx.borrow().clone();
            if let Err(err) = state {
                debug!("process {} has terminated", self.pid);
                break;
            }
        }
        match ret {
            Some(a) => {
                debug!("process {} exited with code {}", self.pid, a);
                a
            }
            None => {
                debug!("process {} silently exited", self.pid);
                ERR_PANIC
            }
        }
    }

    pub fn terminate(&mut self, exit_code: i32) {
        self.exit_tx.send(Some(exit_code));
    }
}
