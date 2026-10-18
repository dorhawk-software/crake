//! What a system command says, for a report's parts (`report::parts`):
//! stdout and stderr in the C locale, line by line, in a time limit; a
//! command that's missing or doesn't finish says so in its place.

use std::time::Duration;

/// How long one command may take (PowerShell's first start is slow).
const LIMIT: Duration = Duration::from_secs(20);

/// `program args…`'s output, its lines.
pub async fn command_lines(program: &str, args: &[&str]) -> Vec<String> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        /// No console window for it.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let shown = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    match tokio::time::timeout(LIMIT, cmd.output()).await {
        Ok(Ok(out)) => {
            let text = String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr);
            text.lines().map(str::to_owned).collect()
        }
        Ok(Err(e)) => vec![format!("({shown}: {e})")],
        Err(_) => vec![format!("({shown} didn't finish in {} s)", LIMIT.as_secs())],
    }
}
