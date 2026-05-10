/// Commands sent to the single recording consumer task to ensure
/// start/stop are processed sequentially (no race conditions).
pub enum RecordingCommand {
    Start,
    Stop,
    Toggle,
}
