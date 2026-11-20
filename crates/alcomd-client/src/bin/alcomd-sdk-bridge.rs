fn main() -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(alcomd_client::sdk_stdio::run());
    // Tokio's stdin adapter may still be waiting on the parent after an event
    // stream ends. Process exit releases the private pipe without waiting on it.
    runtime.shutdown_timeout(std::time::Duration::ZERO);
    result
}
