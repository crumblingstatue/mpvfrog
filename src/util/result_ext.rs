use std::fmt::Display;

pub trait LogErrExt {
    fn log_err(self, prefix: &str);
}

impl<E> LogErrExt for Result<(), E>
where
    E: Display,
{
    fn log_err(self, prefix: &str) {
        match self {
            Ok(()) => todo!(),
            Err(e) => {
                crate::logln!("{prefix}: {e}");
            }
        }
    }
}
