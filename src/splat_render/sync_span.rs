use burn::tensor::backend::Backend;
use tracing::{info_span, span::EnteredSpan};

pub struct SyncSpan<'a, B: Backend> {
    span: EnteredSpan,
    device: &'a B::Device,
}

impl<'a, B: Backend> SyncSpan<'a, B> {
    pub fn new(name: &'static str, device: &'a B::Device) -> Self {
        let span = info_span!("sync", name).entered();
        Self { span, device }
    }
}

impl<'a, B: Backend> Drop for SyncSpan<'a, B> {
    fn drop(&mut self) {
        #[cfg(feature = "tracy")]
        B::sync(self.device);
    }
}