use std::fmt;

pub struct FfmpegTimeFmt(pub f64);

impl fmt::Display for FfmpegTimeFmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0;
        let hh = secs / 3600.0;
        let mm = hh.fract() * 60.0;
        let ss = mm.fract() * 60.0;
        write!(
            f,
            "{:02.0}:{:02.0}:{:02.0}.{:03}",
            hh.floor(),
            mm.floor(),
            ss.floor(),
            (ss.fract() * 1000.0).round() as u64
        )
    }
}

#[test]
fn test_time_fmt() {
    assert_eq!(&FfmpegTimeFmt(0.0).to_string()[..], "00:00:00.000");
    assert_eq!(&FfmpegTimeFmt(24.56).to_string()[..], "00:00:24.560");
    assert_eq!(&FfmpegTimeFmt(119.885).to_string()[..], "00:01:59.885");
    assert_eq!(&FfmpegTimeFmt(52349.345).to_string()[..], "14:32:29.345");
}
