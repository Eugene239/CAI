use std::{
    error::Error,
    fmt, fs,
    path::{Component, Path},
};

use crate::envelope::SessionFile;

#[derive(Debug)]
pub struct ExecutorError(String);

impl fmt::Display for ExecutorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ExecutorError {}

pub fn validate_result_output_path(
    task_directory: &Path,
    result_output: &Path,
) -> Result<(), ExecutorError> {
    if result_output.starts_with(task_directory) {
        return Err(ExecutorError(
            "executor result output must be outside the task directory".to_owned(),
        ));
    }

    Ok(())
}

pub fn materialize_session_files(
    task_directory: &Path,
    session_files: &[SessionFile],
) -> Result<(), ExecutorError> {
    for session_file in session_files {
        if !is_single_file_name(&session_file.name) {
            return Err(ExecutorError(
                "session file name must be a single non-empty path component".to_owned(),
            ));
        }
    }

    #[cfg(unix)]
    let directory_creation = {
        use std::os::unix::fs::DirBuilderExt;

        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(task_directory)
    };
    #[cfg(not(unix))]
    let directory_creation = fs::create_dir(task_directory);

    directory_creation.map_err(|error| {
        ExecutorError(format!("could not create task session directory: {error}"))
    })?;
    for session_file in session_files {
        fs::write(
            task_directory.join(&session_file.name),
            &session_file.contents,
        )
        .map_err(|error| {
            ExecutorError(format!("could not materialize task session file: {error}"))
        })?;
    }

    Ok(())
}

fn is_single_file_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}
