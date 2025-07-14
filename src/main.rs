use std::{fs::File, io::{Read, Write}, os::unix::fs::OpenOptionsExt, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use daemonize_me::Daemon;

fn start_daemon(
    name: &str,
    command: &str,
    args: &[&str],
    stdin: File,
    stdout: File,
    stderr: File,
) -> Result<()> {
    let pid_file_path = format!("{}/{}.pid", PIPE_DIR, name);

    Daemon::new()
        .pid_file(pid_file_path, Some(false))
        .work_dir(".")
        .start()?;

    let (pty, pts) = pty_process::blocking::open()?;
    pty.resize(pty_process::Size::new(24, 80))?;
    let mut child = pty_process::blocking::Command::new(command)
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(stderr)
        .spawn(pts)?;
    child.wait()?;
    Ok(())
}

const PIPE_DIR: &str = "/tmp/daemon_pipes";

fn create_files(name: &str) -> Result<()> {
    let stdin_path = format!("{}/{}_stdin", PIPE_DIR, name);
    let stdout_path = format!("{}/{}_stdout", PIPE_DIR, name);
    let stderr_path = format!("{}/{}_stderr", PIPE_DIR, name);

    ensure_pipe_dir_exists()?;

    interprocess::os::unix::fifo_file::create_fifo(stdin_path, 0o777)?;
    interprocess::os::unix::fifo_file::create_fifo(stdout_path, 0o777)?;
    interprocess::os::unix::fifo_file::create_fifo(stderr_path, 0o777)?;

    Ok(())
}

fn get_files(name: &str) -> Result<(File, File, File)> {
    let stdin_path = format!("{}/{}_stdin", PIPE_DIR, name);
    let stdout_path = format!("{}/{}_stdout", PIPE_DIR, name);
    let stderr_path = format!("{}/{}_stderr", PIPE_DIR, name);

    let stdin = File::options().read(true).write(true).open(stdin_path)?;
    let stdout = File::options()
        .read(true)
        .write(true)
        .append(true)
        .open(stdout_path)?;
    let stderr = File::options()
        .read(true)
        .write(true)
        .append(true)
        .open(stderr_path)?;
    Ok((stdin, stdout, stderr))
}

fn ensure_pipe_dir_exists() -> Result<()> {
    std::fs::create_dir_all(PIPE_DIR)?;
    Ok(())
}

fn create(name: &str, command: &str, args: &[&str]) -> Result<()> {
    create_files(name)?;
    let (stdin, stdout, stderr) = get_files(name)?;

    start_daemon(name, command, args, stdin, stdout, stderr)?;

    Ok(())
}

fn ensure_pid_file(name: &str) -> Result<()> {
    let pid_file_path = format!("{}/{}.pid", PIPE_DIR, name);
    if !PathBuf::from(&pid_file_path).exists() {
        return Err(anyhow::anyhow!(
            "PID file does not exist: {}",
            pid_file_path
        ));
    }
    Ok(())
}

fn write(name: &str, message: &str) -> Result<()> {
    ensure_process_is_running(name)?;
    let stdin_path = format!("{}/{}_stdin", PIPE_DIR, name);
    let mut file = File::options().write(true).open(stdin_path)?;
    file.write_all(message.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn read_stdout(name: &str) -> Result<String> {
    ensure_process_is_running(name)?;
    let stdout_path = format!("{}/{}_stdout", PIPE_DIR, name);
    let mut stdout = File::options().read(true).custom_flags(libc::O_NONBLOCK).open(stdout_path)?;
    let mut output = String::new();
    stdout.read_to_string(&mut output).ok();
    Ok(output)
}

fn read_stderr(name: &str) -> Result<String> {
    ensure_process_is_running(name)?;
    let stderr_path = format!("{}/{}_stderr", PIPE_DIR, name);
    let output = std::fs::read_to_string(stderr_path)?;
    Ok(output)
}

fn list_daemons() -> Result<Vec<String>> {
    ensure_pipe_dir_exists()?;
    let mut daemons = Vec::new();
    for entry in std::fs::read_dir(PIPE_DIR)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.file_name().to_str().unwrap().ends_with(".pid") {
            daemons.push(entry.file_name().to_str().unwrap().replace(".pid", ""));
        }
    }
    Ok(daemons)
}

fn ensure_process_is_running(name: &str) -> Result<()> {
    ensure_pid_file(name)?;
    let pid_file_path = format!("{}/{}.pid", PIPE_DIR, name);
    let pid: i32 = std::fs::read_to_string(&pid_file_path)?.trim().parse()?;
    if unsafe { libc::kill(pid, 0) } != 0 {
        return Err(anyhow::anyhow!("Process {} is not running", name));
    }
    Ok(())
}

fn kill_daemon(name: &str) -> Result<()> {
    ensure_pid_file(name)?;
    let pid_file_path = format!("{}/{}.pid", PIPE_DIR, name);
    let pid: i32 = std::fs::read_to_string(&pid_file_path)?.trim().parse()?;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }

    std::fs::remove_file(pid_file_path)?;
    let stdin_path = format!("{}/{}_stdin", PIPE_DIR, name);
    let stdout_path = format!("{}/{}_stdout", PIPE_DIR, name);
    let stderr_path = format!("{}/{}_stderr", PIPE_DIR, name);
    std::fs::remove_file(stdin_path)?;
    std::fs::remove_file(stdout_path)?;
    std::fs::remove_file(stderr_path)?;

    Ok(())
}

#[derive(Parser)]
#[command(name = "attyvo")]
#[command(about = "A daemon process manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new daemon process
    Create {
        /// Name of the daemon
        name: String,
        /// Command to execute
        command: String,
        /// Arguments for the command
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Write to daemon's stdin
    Write {
        /// Name of the daemon
        name: String,
        /// Message to write
        message: String,
    },
    /// Read from daemon's stdin (note: this reads from stdin pipe)
    ReadStderr {
        /// Name of the daemon
        name: String,
    },
    /// Read from daemon's stdout
    Read {
        /// Name of the daemon
        name: String,
    },
    /// Kill a daemon process
    Kill {
        /// Name of the daemon
        name: String,
    },
    /// List all running daemons
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create {
            name,
            command,
            args,
        } => {
            let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            create(&name, &command, &args_refs)?;
            println!("Daemon '{}' created and started", name);
        }
        Commands::Write { name, message } => {
            write(&name, &message)?;
            println!("Message written to daemon '{}'", name);
        }
        Commands::ReadStderr { name } => {
            let output = read_stderr(&name)?;
            print!("{}", output);
        }
        Commands::Read { name } => {
            let output = read_stdout(&name)?;
            print!("{}", output);
        }
        Commands::Kill { name } => {
            kill_daemon(&name)?;
            println!("Daemon '{}' killed", name);
        }
        Commands::List => {
            let daemons = list_daemons()?;
            if daemons.is_empty() {
                println!("No running daemons");
            } else {
                println!("Running daemons:");
                for daemon in daemons {
                    println!("  - {}", daemon);
                }
            }
        }
    }

    Ok(())
}
