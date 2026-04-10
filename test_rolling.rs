use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};

fn main() {
    let _ = BasicRollingFileAppender::new(
        "test.log",
        RollingConditionBasic::new().daily().max_size(1024),
        3,
    ).unwrap();
}
