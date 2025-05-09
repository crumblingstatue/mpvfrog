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

pub trait ResultModalExt {
    fn err_popup(&self, title: &str, modal: &mut crate::app::ModalPopup);
}

impl<T, E: Display> ResultModalExt for Result<T, E> {
    fn err_popup(&self, title: &str, modal: &mut crate::app::ModalPopup) {
        if let Err(e) = self {
            modal.error(title, e);
        }
    }
}
