use std::fmt::Display;

pub trait LogErrExt {
    fn log_err(self, prefix: &str);
}

impl<E> LogErrExt for Result<(), E>
where
    E: Display,
{
    fn log_err(self, prefix: &str) {
        if let Err(e) = self {
            crate::logln!("{prefix}: {e}");
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

impl<T, E: Display> ResultModalExt for Option<Result<T, E>> {
    fn err_popup(&self, title: &str, modal: &mut crate::app::ModalPopup) {
        if let Some(Err(e)) = self {
            modal.error(title, e);
        }
    }
}
