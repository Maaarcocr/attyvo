# attyvo

A tool that enables non-interactive programs to interact with interactive CLIs by providing a pseudo-terminal (PTY) interface.

## Overview

attyvo acts as a bridge between programs that expect terminal input/output and non-interactive environments. This is particularly useful for AI coding assistants like Claude Code, which need to interact with interactive command-line tools programmatically.

## Installation

```bash
cargo install attyvo
```

## Features

- Creates pseudo-terminals for interactive CLI programs
- Manages multiple daemon processes
- Handles bidirectional communication between non-interactive callers and interactive programs
- Supports process lifecycle management (start, stop, kill-all)

## Use Cases

- Enabling AI tools to interact with interactive CLIs (e.g., database shells, REPLs)
- Automating interactive command-line workflows
- Testing interactive CLI applications

## Commands

- `start` - Start a new daemon process
- `stop` - Stop a running daemon
- `kill-all` - Terminate all running daemons
- `list` - List all running daemons
- `send` - Send input to a daemon
- `read` - Read output from a daemon

## Why attyvo?

Many command-line tools detect whether they're running in a terminal and change their behavior accordingly. Without a proper PTY, these tools may refuse to run interactively or provide limited functionality. attyvo solves this by providing a real PTY interface, making it possible for automated tools and AI assistants to interact with any CLI program as if a human were typing at a terminal.