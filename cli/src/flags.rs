use std::env;

pub struct Flags {
    pub json: bool,
    pub full: bool,
    pub headed: bool,
    pub debug: bool,
    pub session: String,
    pub headers: Option<String>,
    pub executable_path: Option<String>,
    pub cdp: Option<String>,
    pub extensions: Vec<String>,
    pub channel: Option<String>,
    pub tab_name: Option<String>,
}

pub fn parse_flags(args: &[String]) -> Flags {
    let extensions_env = env::var("AGENT_BROWSER_EXTENSIONS")
        .ok()
        .map(|s| s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();

    let headed_env = env::var("AGENT_BROWSER_HEADED")
        .ok()
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    let mut flags = Flags {
        json: false,
        full: false,
        headed: headed_env,
        debug: false,
        session: env::var("AGENT_BROWSER_SESSION").unwrap_or_else(|_| "main".to_string()),
        headers: None,
        executable_path: env::var("AGENT_BROWSER_EXECUTABLE_PATH").ok(),
        cdp: None,
        extensions: extensions_env,
        channel: env::var("AGENT_BROWSER_CHANNEL").ok(),
        tab_name: env::var("AGENT_BROWSER_TABNAME").ok(),
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => flags.json = true,
            "--full" | "-f" => flags.full = true,
            "--headed" => {
                // --headed (default true), --headed true, --headed false
                if let Some(next) = args.get(i + 1) {
                    match next.as_str() {
                        "false" => { flags.headed = false; i += 1; }
                        "true" => { flags.headed = true; i += 1; }
                        _ => flags.headed = true, // --headed without value
                    }
                } else {
                    flags.headed = true;
                }
            }
            "--debug" => flags.debug = true,
            "--session" => {
                if let Some(s) = args.get(i + 1) {
                    flags.session = s.clone();
                    i += 1;
                }
            }
            "--headers" => {
                if let Some(h) = args.get(i + 1) {
                    flags.headers = Some(h.clone());
                    i += 1;
                }
            }
            "--executable-path" => {
                if let Some(s) = args.get(i + 1) {
                    flags.executable_path = Some(s.clone());
                    i += 1;
                }
            },
            "--extension" => {
                if let Some(s) = args.get(i + 1) {
                    flags.extensions.push(s.clone());
                    i += 1;
                }
            },
            "--cdp" => {
                if let Some(s) = args.get(i + 1) {
                    flags.cdp = Some(s.clone());
                    i += 1;
                }
            }
            "--channel" => {
                if let Some(c) = args.get(i + 1) {
                    flags.channel = Some(c.clone());
                    i += 1;
                }
            }
            "--tabname" => {
                if let Some(name) = args.get(i + 1) {
                    flags.tab_name = Some(name.clone());
                    i += 1;
                }
            }
            "--bundled" => {
                // Use Patchright's bundled Chrome for Testing instead of system Chrome
                flags.channel = Some("bundled".to_string());
            }
            _ => {}
        }
        i += 1;
    }
    flags
}

pub fn clean_args(args: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut skip_next = false;

    // Global flags that should be stripped from command args
    const GLOBAL_FLAGS: &[&str] = &["--json", "--full", "--debug", "--bundled"];
    // Global flags that take a value (need to skip the next arg too)
    const GLOBAL_FLAGS_WITH_VALUE: &[&str] = &["--session", "--headers", "--executable-path", "--cdp", "--extension", "--channel", "--tabname"];

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if skip_next {
            skip_next = false;
            i += 1;
            continue;
        }
        if GLOBAL_FLAGS_WITH_VALUE.contains(&arg.as_str()) {
            skip_next = true;
            i += 1;
            continue;
        }
        // --headed with optional true/false value
        if arg == "--headed" {
            if let Some(next) = args.get(i + 1) {
                if next == "true" || next == "false" {
                    i += 2; // skip --headed and its value
                    continue;
                }
            }
            i += 1; // skip --headed alone
            continue;
        }
        // Only strip known global flags, not command-specific flags
        if GLOBAL_FLAGS.contains(&arg.as_str()) || arg == "-f" {
            i += 1;
            continue;
        }
        result.push(arg.clone());
        i += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn test_parse_headers_flag() {
        let flags = parse_flags(&args(r#"open example.com --headers {"Auth":"token"}"#));
        assert_eq!(flags.headers, Some(r#"{"Auth":"token"}"#.to_string()));
    }

    #[test]
    fn test_parse_headers_flag_with_spaces() {
        // Headers JSON is passed as a single quoted argument in shell
        let input: Vec<String> = vec![
            "open".to_string(),
            "example.com".to_string(),
            "--headers".to_string(),
            r#"{"Authorization": "Bearer token"}"#.to_string(),
        ];
        let flags = parse_flags(&input);
        assert_eq!(flags.headers, Some(r#"{"Authorization": "Bearer token"}"#.to_string()));
    }

    #[test]
    fn test_parse_no_headers_flag() {
        let flags = parse_flags(&args("open example.com"));
        assert!(flags.headers.is_none());
    }

    #[test]
    fn test_clean_args_removes_headers() {
        let input: Vec<String> = vec![
            "open".to_string(),
            "example.com".to_string(),
            "--headers".to_string(),
            r#"{"Auth":"token"}"#.to_string(),
        ];
        let clean = clean_args(&input);
        assert_eq!(clean, vec!["open", "example.com"]);
    }

    #[test]
    fn test_clean_args_removes_headers_at_start() {
        let input: Vec<String> = vec![
            "--headers".to_string(),
            r#"{"Auth":"token"}"#.to_string(),
            "open".to_string(),
            "example.com".to_string(),
        ];
        let clean = clean_args(&input);
        assert_eq!(clean, vec!["open", "example.com"]);
    }

    #[test]
    fn test_headers_with_other_flags() {
        let input: Vec<String> = vec![
            "open".to_string(),
            "example.com".to_string(),
            "--headers".to_string(),
            r#"{"Auth":"token"}"#.to_string(),
            "--json".to_string(),
            "--headed".to_string(),
        ];
        let flags = parse_flags(&input);
        assert_eq!(flags.headers, Some(r#"{"Auth":"token"}"#.to_string()));
        assert!(flags.json);
        assert!(flags.headed);
        
        let clean = clean_args(&input);
        assert_eq!(clean, vec!["open", "example.com"]);
    }

    #[test]
    fn test_parse_executable_path_flag() {
        let flags = parse_flags(&args("--executable-path /path/to/chromium open example.com"));
        assert_eq!(flags.executable_path, Some("/path/to/chromium".to_string()));
    }

    #[test]
    fn test_parse_executable_path_flag_no_value() {
        let flags = parse_flags(&args("--executable-path"));
        assert_eq!(flags.executable_path, None);
    }

    #[test]
    fn test_clean_args_removes_executable_path() {
        let cleaned = clean_args(&args("--executable-path /path/to/chromium open example.com"));
        assert_eq!(cleaned, vec!["open", "example.com"]);
    }

    #[test]
    fn test_clean_args_removes_executable_path_with_other_flags() {
        let cleaned = clean_args(&args("--json --executable-path /path/to/chromium --headed open example.com"));
        assert_eq!(cleaned, vec!["open", "example.com"]);
    }

    #[test]
    fn test_parse_flags_with_session_and_executable_path() {
        let flags = parse_flags(&args("--session test --executable-path /custom/chrome open example.com"));
        assert_eq!(flags.session, "test");
        assert_eq!(flags.executable_path, Some("/custom/chrome".to_string()));
    }

    #[test]
    fn test_parse_channel_flag() {
        let flags = parse_flags(&args("--channel chrome open example.com"));
        assert_eq!(flags.channel, Some("chrome".to_string()));
    }

    #[test]
    fn test_parse_channel_flag_no_value() {
        let flags = parse_flags(&args("--channel"));
        assert_eq!(flags.channel, None);
    }

    #[test]
    fn test_clean_args_removes_channel() {
        let cleaned = clean_args(&args("--channel chrome open example.com"));
        assert_eq!(cleaned, vec!["open", "example.com"]);
    }

    #[test]
    fn test_parse_channel_with_other_flags() {
        let flags = parse_flags(&args("--channel chrome --headed --json open example.com"));
        assert_eq!(flags.channel, Some("chrome".to_string()));
        assert!(flags.headed);
        assert!(flags.json);
    }

    #[test]
    fn test_parse_bundled_flag() {
        let flags = parse_flags(&args("--bundled open example.com"));
        assert_eq!(flags.channel, Some("bundled".to_string()));
    }

    #[test]
    fn test_clean_args_removes_bundled() {
        let cleaned = clean_args(&args("--bundled open example.com"));
        assert_eq!(cleaned, vec!["open", "example.com"]);
    }

    #[test]
    fn test_bundled_with_other_flags() {
        let flags = parse_flags(&args("--bundled --headed --json open example.com"));
        assert_eq!(flags.channel, Some("bundled".to_string()));
        assert!(flags.headed);
        assert!(flags.json);
    }
}
