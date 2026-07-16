pub(crate) fn secret_file_read_error(error: secret_store::SecretError) -> anyhow::Error {
    let is_unsafe_file = error.is_unsafe_file();
    let error = anyhow::Error::new(error);
    if is_unsafe_file {
        error.context("not a regular secret file")
    } else {
        error
    }
}
