#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    burst_frame_deduplicator::linux_gui::run()
}

#[cfg(not(target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("The GTK application is available only on Linux")
}
