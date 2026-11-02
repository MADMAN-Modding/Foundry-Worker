use std::{error::Error, fmt};

use log::{Level, info, log, warn};

/// The level and text `log_result` emits for a result
pub fn result_message<V, E>(result: &Result<V, E>, message: &str) -> (Level, String)
where
    E: fmt::Display + Error,
{
    match result {
        Ok(_) => (Level::Info, message.to_string()),
        Err(e) => (Level::Warn, format!("Failed to {}: {}", message, e)),
    }
}

pub fn log_result<V, E>(result: &Result<V, E>, message: &str)
where
    E: fmt::Display + Error,
{
    let (level, text) = result_message(result, message);

    log!(level, "{}", text);
}

pub fn log_option<V>(option: &Option<V>, message: &str) {
    match option {
        Some(_) => info!("{}", message),
        None => warn!("Failed to: {}", message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Boom;

    impl fmt::Display for Boom {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "boom")
        }
    }

    impl Error for Boom {}

    #[test]
    fn ok_results_log_the_plain_message_at_info() {
        let result: Result<(), Boom> = Ok(());

        assert_eq!(
            result_message(&result, "Load Config"),
            (Level::Info, "Load Config".to_string())
        );
    }

    #[test]
    fn errors_log_the_message_and_cause_at_warn() {
        let result: Result<(), Boom> = Err(Boom);

        assert_eq!(
            result_message(&result, "Load Config"),
            (Level::Warn, "Failed to Load Config: boom".to_string())
        );
    }

    #[test]
    fn logging_helpers_do_not_panic() {
        log_result(&Ok::<(), Boom>(()), "ok");
        log_result(&Err::<(), Boom>(Boom), "err");
        log_option(&Some(1), "some");
        log_option(&None::<i32>, "none");
    }
}