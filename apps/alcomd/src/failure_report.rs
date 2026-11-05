//! Original typed failures are reported before public error sanitization.
pub(super) fn report(
    error: &(dyn std::error::Error + 'static),
    path: Option<&str>,
    message: &'static str,
) -> String {
    alcomd_application::record_technical_failure(error, path, None, message)
}
