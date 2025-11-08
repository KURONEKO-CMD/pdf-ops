use std::env;

pub fn sanitize_path_input(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        if s.len() >= 2 { s = s[1..s.len()-1].to_string(); }
    }
    // expand ~ to home
    if s.starts_with('~') {
        let rest = &s[1..];
        let home = if cfg!(windows) {
            env::var("USERPROFILE").unwrap_or_default()
        } else {
            env::var("HOME").unwrap_or_default()
        };
        if !home.is_empty() {
            if rest.is_empty() { s = home; }
            else if rest.starts_with(['/', '\\']) { s = format!("{}{}", home, rest); }
        }
    }
    if !cfg!(windows) {
        // unescape common shell-escaped spaces
        s = s.replace("\\ ", " ");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::sanitize_path_input;
    use std::env;

    #[test]
    fn quotes_and_escaped_spaces() {
        assert_eq!(sanitize_path_input("\"/a/b c\""), "/a/b c");
        assert_eq!(sanitize_path_input("'/a/b c'"), "/a/b c");
        if !cfg!(windows) {
            assert_eq!(sanitize_path_input("/a/b\\ c"), "/a/b c");
        }
    }

    #[test]
    fn tilde_expansion() {
        let tmp = tempfile::tempdir().unwrap();
        if cfg!(windows) { env::set_var("USERPROFILE", tmp.path()); }
        else { env::set_var("HOME", tmp.path()); }
        assert_eq!(sanitize_path_input("~"), tmp.path().to_string_lossy());
        let expect = format!("{}/sub", tmp.path().to_string_lossy());
        assert_eq!(sanitize_path_input("~/sub"), expect);
    }

    #[test]
    fn windows_backslashes_kept() {
        // ensure we don't mangle generic backslashes that are not escapes
        let p = "C:\\Program Files\\App\\file.pdf";
        assert_eq!(sanitize_path_input(p), p);
    }
}

