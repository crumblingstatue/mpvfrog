pub trait BoolExt {
    fn take(&mut self) -> bool;
}

impl BoolExt for bool {
    fn take(&mut self) -> bool {
        std::mem::take(self)
    }
}
