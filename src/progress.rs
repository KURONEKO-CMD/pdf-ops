use std::borrow::Cow;

pub trait ProgressSink {
    fn set_len(&self, _len: u64) {}
    fn inc(&self, _n: u64) {}
    fn set_message(&self, _msg: Cow<'static, str>) {}
    fn finish(&self, _msg: Cow<'static, str>) {}
}

#[allow(dead_code)]
pub struct NullProgress;
impl ProgressSink for NullProgress {}

pub struct IndicatifProgress {
    pb: indicatif::ProgressBar,
}

impl IndicatifProgress {
    pub fn new() -> Self {
        let pb = indicatif::ProgressBar::new(0);
        // simple style; caller can customize later if needed
        let _ = pb.set_style(
            indicatif::ProgressStyle::with_template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        Self { pb }
    }
}

impl ProgressSink for IndicatifProgress {
    fn set_len(&self, len: u64) { self.pb.set_length(len); }
    fn inc(&self, n: u64) { self.pb.inc(n); }
    fn set_message(&self, msg: Cow<'static, str>) { self.pb.set_message(msg); }
    fn finish(&self, msg: Cow<'static, str>) { self.pb.finish_with_message(msg); }
}
