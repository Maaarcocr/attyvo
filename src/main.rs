use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use daemonize_me::Daemon;

fn start_daemon(name: &str, stdin: File, stdout: File, stderr: File) -> Result<()> {
    let pid_file_path = format!("{}/{}", PIPE_DIR, name);
    Daemon::new()
        .pid_file(pid_file_path, Some(false))
        .stdin(stdin)
        .stdout(stdout)
        .stderr(stderr)
        .umask(0o000)
        .work_dir(".")
        .start()?;

    Ok(())
}

const PIPE_DIR: &str = "/tmp/daemon_pipes";

fn create_named_pipes(name: &str) -> Result<()> {
    let stdin_path = format!("{}/{}_stdin", PIPE_DIR, name);
    let stdout_path = format!("{}/{}_stdout", PIPE_DIR, name);
    let stderr_path = format!("{}/{}_stderr", PIPE_DIR, name);

    ensure_pipe_dir_exists()?;

    unix_named_pipe::create(stdin_path, None)?;
    unix_named_pipe::create(stdout_path, None)?;
    unix_named_pipe::create(stderr_path, None)?;

    Ok(())
}

fn get_socket_files(name: &str) -> Result<(File, File, File)> {
    let stdin_path = format!("{}/{}_stdin", PIPE_DIR, name);
    let stdout_path = format!("{}/{}_stdout", PIPE_DIR, name);
    let stderr_path = format!("{}/{}_stderr", PIPE_DIR, name);

    let stdin = unix_named_pipe::open_read(stdin_path)?;
    let stdout = unix_named_pipe::open_write(stdout_path)?;
    let stderr = unix_named_pipe::open_write(stderr_path)?;
    Ok((stdin, stdout, stderr))
}

fn ensure_pipe_dir_exists() -> Result<()> {
    std::fs::create_dir_all(PIPE_DIR)?;
    Ok(())
}

fn create(name: &str, command: &str, args: &[&str]) -> Result<()> {
    create_named_pipes(name)?;
    let (stdin, stdout, stderr) = get_socket_files(name)?;

    start_daemon(name, stdin, stdout, stderr)?;

    let err = exec::execvp(command, args);
    println!("Failed to execute command: {}", err);

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
    ensure_pid_file(name)?;
    let (stdin, _, _) = get_socket_files(name)?;
    let mut writer = std::io::BufWriter::new(stdin);
    writer.write_all(message.as_bytes())?;
    writer.flush()?;
    Ok(())
}

fn read_stdout(name: &str) -> Result<String> {
    ensure_pid_file(name)?;
    let (_, stdout, _) = get_socket_files(name)?;
    let mut reader = std::io::BufReader::new(stdout);
    let mut output = String::new();
    reader.read_to_string(&mut output)?;
    Ok(output)
}

fn read_stderr(name: &str) -> Result<String> {
    ensure_pid_file(name)?;
    let (_, _, stderr) = get_socket_files(name)?;
    let mut reader = std::io::BufReader::new(stderr);
    let mut output = String::new();
    reader.read_to_string(&mut output)?;
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

fn kill_daemon(name: &str) -> Result<()> {
    ensure_pid_file(name)?;
    let pid_file_path = format!("{}/{}.pid", PIPE_DIR, name);
    let pid: i32 = std::fs::read_to_string(pid_file_path)?.trim().parse()?;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
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
    ReadStdout {
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
        Commands::ReadStdout { name } => {
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
