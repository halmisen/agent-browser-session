mod commands;
mod connection;
mod flags;
mod install;
mod output;

use serde_json::json;
use std::env;
use std::fs;
use std::process::exit;

#[cfg(unix)]
use libc;

#[cfg(windows)]
use windows_sys::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use commands::{gen_id, parse_command, ParseError};
use connection::{ensure_daemon, get_socket_dir, send_command};
use flags::{clean_args, parse_flags};
use install::run_install;
use output::{print_command_help, print_help, print_response};

fn run_session(args: &[String], session: &str, json_mode: bool) {
    let subcommand = args.get(1).map(|s| s.as_str());

    match subcommand {
        Some("list") => {
            let socket_dir = get_socket_dir();
            let mut sessions: Vec<String> = Vec::new();

            if let Ok(entries) = fs::read_dir(&socket_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Look for pid files (new format: {session}.pid)
                    if name.ends_with(".pid") {
                        let session_name = name
                            .strip_suffix(".pid")
                            .unwrap_or("");
                        if !session_name.is_empty() {
                            // Check if session is actually running
                            let pid_path = socket_dir.join(&name);
                            if let Ok(pid_str) = fs::read_to_string(&pid_path) {
                                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                    #[cfg(unix)]
                                    let running = unsafe { libc::kill(pid as i32, 0) == 0 };
                                    #[cfg(windows)]
                                    let running = unsafe {
                                        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                                        if handle != 0 {
                                            CloseHandle(handle);
                                            true
                                        } else {
                                            false
                                        }
                                    };
                                    if running {
                                        sessions.push(session_name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if json_mode {
                println!(
                    r#"{{"success":true,"data":{{"sessions":{}}}}}"#,
                    serde_json::to_string(&sessions).unwrap_or_default()
                );
            } else if sessions.is_empty() {
                println!("No active sessions");
            } else {
                println!("Active sessions:");
                for s in &sessions {
                    let marker = if s == session { "→" } else { " " };
                    println!("{} {}", marker, s);
                }
            }
        }
        None | Some(_) => {
            // Just show current session
            if json_mode {
                println!(r#"{{"success":true,"data":{{"session":"{}"}}}}"#, session);
            } else {
                println!("{}", session);
            }
        }
    }
}

/// Find SKILL.md relative to the binary location.
/// Checks multiple paths to support different installation methods:
/// - Development: binary is in cli/target/release/, skills/ is at repo root
/// - Homebrew: binary is in bin/, skills/ is alongside or in libexec/
/// - Environment override: AGENT_BROWSER_SKILLS_DIR
fn find_skill_file() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    let skill_name = "agent-browser-session/SKILL.md";

    // Check environment variable first
    if let Ok(skills_dir) = env::var("AGENT_BROWSER_SKILLS_DIR") {
        let p = PathBuf::from(skills_dir).join(skill_name);
        if p.exists() {
            return Some(p);
        }
    }

    // Relative to the executable
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidates = [
                // Development: exe is in cli/target/release/
                exe_dir.join("../../../skills").join(skill_name),
                // Installed: exe is in bin/, skills/ is sibling
                exe_dir.join("../skills").join(skill_name),
                // Same directory
                exe_dir.join("skills").join(skill_name),
                // Homebrew libexec layout
                exe_dir.join("../libexec/skills").join(skill_name),
            ];
            for candidate in &candidates {
                if let Ok(canonical) = candidate.canonicalize() {
                    if canonical.exists() {
                        return Some(canonical);
                    }
                }
            }
        }
    }

    None
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let flags = parse_flags(&args);
    let clean = clean_args(&args);

    let has_help = args.iter().any(|a| a == "--help" || a == "-h");
    let has_version = args.iter().any(|a| a == "--version" || a == "-V");

    if has_version {
        println!("agent-browser-session {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if clean.is_empty() {
        print_help();
        return;
    }

    if has_help {
        if let Some(cmd) = clean.get(0) {
            if print_command_help(cmd) {
                return;
            }
        }
        print_help();
        return;
    }

    // Handle install separately
    if clean.get(0).map(|s| s.as_str()) == Some("install") {
        let with_deps = args.iter().any(|a| a == "--with-deps" || a == "-d");
        run_install(with_deps);
        return;
    }

    // Handle kill — terminate all daemon processes and clean up
    // This is for manual use only, NOT for agents (shared browser across tabnames)
    if clean.get(0).map(|s| s.as_str()) == Some("kill") {
        let socket_dir = get_socket_dir();
        let mut killed = 0u32;

        if let Ok(entries) = fs::read_dir(&socket_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".pid") {
                    let pid_path = socket_dir.join(&name);
                    if let Ok(pid_str) = fs::read_to_string(&pid_path) {
                        if let Ok(pid) = pid_str.trim().parse::<i32>() {
                            #[cfg(unix)]
                            {
                                // Kill the daemon process and its children (browser)
                                unsafe {
                                    libc::kill(-pid, libc::SIGTERM); // kill process group
                                    libc::kill(pid, libc::SIGTERM);
                                }
                            }
                            #[cfg(windows)]
                            {
                                let _ = std::process::Command::new("taskkill")
                                    .args(&["/PID", &pid.to_string(), "/T", "/F"])
                                    .output();
                            }
                            killed += 1;
                        }
                    }
                    let _ = fs::remove_file(&pid_path);
                }
                // Clean up socket files
                if name.ends_with(".sock") {
                    let _ = fs::remove_file(socket_dir.join(&name));
                }
            }
        }

        if killed > 0 {
            eprintln!("\x1b[32m✓\x1b[0m Killed {} daemon process(es) and cleaned up socket files", killed);
        } else {
            eprintln!("No running daemons found");
        }
        eprintln!("\x1b[2m  Note: This is for manual use only. Agents should NOT call kill — the browser is shared across tabnames.\x1b[0m");
        return;
    }

    // Handle install-skills — copy SKILL.md to ~/.claude/skills/
    if clean.get(0).map(|s| s.as_str()) == Some("install-skills") {
        let skill_src = find_skill_file();
        match skill_src {
            Some(src) => {
                let home = dirs::home_dir().expect("Cannot find home directory");
                let dest_dir = home.join(".claude").join("skills").join("agent-browser-session");
                fs::create_dir_all(&dest_dir).expect("Cannot create skills directory");
                let dest = dest_dir.join("SKILL.md");
                fs::copy(&src, &dest).expect("Cannot copy skill file");
                eprintln!(
                    "\x1b[32m✓\x1b[0m Installed skill to {}",
                    dest.display()
                );
            }
            None => {
                eprintln!("SKILL.md not found locally. Install manually:");
                eprintln!("  mkdir -p ~/.claude/skills/agent-browser-session");
                eprintln!("  curl -sL https://raw.githubusercontent.com/BUNotesAI/agent-browser-session/main/skills/agent-browser-session/SKILL.md \\");
                eprintln!("    -o ~/.claude/skills/agent-browser-session/SKILL.md");
                exit(1);
            }
        }
        return;
    }

    // Handle session separately (doesn't need daemon)
    if clean.get(0).map(|s| s.as_str()) == Some("session") {
        run_session(&clean, &flags.session, flags.json);
        return;
    }

    let mut cmd = match parse_command(&clean, &flags) {
        Ok(c) => c,
        Err(e) => {
            if flags.json {
                let error_type = match &e {
                    ParseError::UnknownCommand { .. } => "unknown_command",
                    ParseError::UnknownSubcommand { .. } => "unknown_subcommand",
                    ParseError::MissingArguments { .. } => "missing_arguments",
                };
                println!(
                    r#"{{"success":false,"error":"{}","type":"{}"}}"#,
                    e.format().replace('\n', " "),
                    error_type
                );
            } else {
                eprintln!("\x1b[31m{}\x1b[0m", e.format());
            }
            exit(1);
        }
    };

    // Every command gets a tabName — default to ZEROTABPAGE for unified routing
    let tab_name = flags.tab_name.as_deref().unwrap_or("ZEROTABPAGE");
    cmd["tabName"] = json!(tab_name);

    let daemon_result = match ensure_daemon(&flags.session, flags.headed, flags.executable_path.as_deref(), &flags.extensions, flags.channel.as_deref()) {
        Ok(result) => result,
        Err(e) => {
            if flags.json {
                println!(r#"{{"success":false,"error":"{}"}}"#, e);
            } else {
                eprintln!("\x1b[31m✗\x1b[0m {}", e);
            }
            exit(1);
        }
    };

    // Warn if executable_path, extensions, or channel was specified but daemon was already running
    if daemon_result.already_running && !flags.json {
        if flags.executable_path.is_some() {
            eprintln!("\x1b[33m⚠\x1b[0m --executable-path ignored: daemon already running. Use 'agent-browser-session close' first to restart with new path.");
        }
        if !flags.extensions.is_empty() {
            eprintln!("\x1b[33m⚠\x1b[0m --extension ignored: daemon already running. Use 'agent-browser-session close' first to restart with extensions.");
        }
        if flags.channel.is_some() {
            eprintln!("\x1b[33m⚠\x1b[0m --channel ignored: daemon already running. Use 'agent-browser-session close' first to restart with new channel.");
        }
    }

    // Connect via CDP if --cdp flag is set
    if let Some(ref port) = flags.cdp {
        let cdp_port: u16 = match port.parse::<u32>() {
            Ok(p) if p == 0 => {
                let msg = "Invalid CDP port: port must be greater than 0".to_string();
                if flags.json {
                    println!(r#"{{"success":false,"error":"{}"}}"#, msg);
                } else {
                    eprintln!("\x1b[31m✗\x1b[0m {}", msg);
                }
                exit(1);
            }
            Ok(p) if p > 65535 => {
                let msg = format!("Invalid CDP port: {} is out of range (valid range: 1-65535)", p);
                if flags.json {
                    println!(r#"{{"success":false,"error":"{}"}}"#, msg);
                } else {
                    eprintln!("\x1b[31m✗\x1b[0m {}", msg);
                }
                exit(1);
            }
            Ok(p) => p as u16,
            Err(_) => {
                let msg = format!("Invalid CDP port: '{}' is not a valid number. Port must be a number between 1 and 65535", port);
                if flags.json {
                    println!(r#"{{"success":false,"error":"{}"}}"#, msg);
                } else {
                    eprintln!("\x1b[31m✗\x1b[0m {}", msg);
                }
                exit(1);
            }
        };

        let mut launch_cmd = json!({
            "id": gen_id(),
            "action": "launch",
            "cdpPort": cdp_port
        });
        launch_cmd["tabName"] = json!(tab_name);

        let err = match send_command(launch_cmd, &flags.session) {
            Ok(resp) if resp.success => None,
            Ok(resp) => Some(resp.error.unwrap_or_else(|| "CDP connection failed".to_string())),
            Err(e) => Some(e.to_string()),
        };

        if let Some(msg) = err {
            if flags.json {
                println!(r#"{{"success":false,"error":"{}"}}"#, msg);
            } else {
                eprintln!("\x1b[31m✗\x1b[0m {}", msg);
            }
            exit(1);
        }
    }

    // Send launch command with headed/headless mode (without CDP)
    if flags.cdp.is_none() {
        let headless = !flags.headed;
        let mut launch_cmd = json!({
            "id": gen_id(),
            "action": "launch",
            "headless": headless
        });
        if let Some(ch) = &flags.channel {
            launch_cmd["channel"] = json!(ch);
        }
        if let Some(path) = &flags.executable_path {
            launch_cmd["executablePath"] = json!(path);
        }
        launch_cmd["tabName"] = json!(tab_name);
        if let Err(e) = send_command(launch_cmd, &flags.session) {
            if !flags.json {
                eprintln!("\x1b[33m⚠\x1b[0m Could not launch browser: {}", e);
            }
        }
    }

    match send_command(cmd, &flags.session) {
        Ok(resp) => {
            let success = resp.success;
            print_response(&resp, flags.json);
            if !success {
                exit(1);
            }
        }
        Err(e) => {
            if flags.json {
                println!(r#"{{"success":false,"error":"{}"}}"#, e);
            } else {
                eprintln!("\x1b[31m✗\x1b[0m {}", e);
            }
            exit(1);
        }
    }
}
