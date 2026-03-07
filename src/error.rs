//! Domain errors for pullhook.

use thiserror::Error;

/// Error type used by internal modules.
#[derive(Debug, Error)]
pub enum PullhookError {
	/// Git command exited unsuccessfully.
	#[error("git command failed: `{command}`\n{stderr}")]
	GitCommand {
		/// Command that was executed.
		command: String,
		/// Stderr output.
		stderr: String,
	},

	/// Git command failed to start.
	#[error("failed to start git command `{command}`: {source}")]
	GitIo {
		/// Command that was executed.
		command: String,
		/// Underlying IO error.
		#[source]
		source: std::io::Error,
	},

	/// Glob pattern parsing or compilation error.
	#[error("invalid pattern `{pattern}`: {reason}")]
	Pattern {
		/// Pattern passed by the user.
		pattern: String,
		/// Human-friendly failure reason.
		reason: String,
	},

	/// Package manager detection found conflicting lock files.
	#[error("multiple package managers detected: {found:?}")]
	AmbiguousPackageManagers {
		/// Detected package manager names.
		found: Vec<&'static str>,
	},

	/// No package manager files were detected.
	#[error("no supported package manager files found in `{root}`")]
	PackageManagerNotFound {
		/// Repo root used for detection.
		root: String,
	},

	/// Command string could not be parsed into argv.
	#[error("invalid command `{command}`: {reason}")]
	CommandParse {
		/// Command string provided by the user.
		command: String,
		/// Parse failure reason.
		reason: String,
	},

	/// Command failed to start.
	#[error("failed to execute `{command}` in `{cwd}`: {source}")]
	CommandIo {
		/// Command string executed.
		command: String,
		/// Current working directory.
		cwd: String,
		/// Underlying IO error.
		#[source]
		source: std::io::Error,
	},

	/// Command exited with non-zero status.
	#[error("command failed: `{command}` in `{cwd}` exited with {status}\n{details}")]
	CommandFailed {
		/// Command string executed.
		command: String,
		/// Current working directory.
		cwd: String,
		/// Exit code when available. `None` means no exit code was provided (e.g. signal termination).
		code: Option<i32>,
		/// Human-readable exit status string.
		status: String,
		/// Captured stderr or failure details.
		details: String,
	},

	/// Generic domain message.
	#[error("{0}")]
	Message(String),
}
