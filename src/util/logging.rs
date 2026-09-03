use std::{error::Error, fmt};

use log::{info, warn};

pub fn log_result<V, E>(result: &Result<V, E>, message: &str)
where E: fmt::Display + Error
{
    match result {
        Ok(_) => info!("{}", message),
        Err(e) => warn!("{}: {}", message, e)
    }
}