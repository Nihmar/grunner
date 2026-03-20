use crate::utils::clipboard::copy_text;
use log::{debug, error, info, warn};
use std::path::Path;

/// Parse a `file:line:content` pattern (like grep -n output)
///
/// Returns (`file_path`, `line_number`) if the input matches "path:line:" format
/// where `line_number` is a positive integer.
pub(crate) fn parse_file_line(line: &str) -> Option<(&str, u32)> {
    // Find the first colon that separates file path from line number
    // We look for pattern: file_path:line_number:rest
    // file_path cannot contain colon on Unix systems
    let mut parts = line.splitn(3, ':');
    let file = parts.next()?;
    if file.is_empty() {
        return None; // File path cannot be empty
    }
    let line_str = parts.next()?;
    // There must be a third part (the content after second colon)
    parts.next()?;

    // Parse line number
    let line_num = line_str.parse::<u32>().ok()?;
    if line_num == 0 {
        return None; // Line numbers start at 1
    }

    Some((file, line_num))
}

/// Open a file or `<file:line>` combination
///
/// # Arguments
/// * `line` - Either a file path or `<file:line>` format
///
/// If no display is available, falls back to silently ignoring clipboard copy.
///
/// If the input matches `<file:line:content>` format (like grep output),
/// opens the file at the specified line using the system EDITOR or xdg-open.
/// If it's just a file path, opens the file.
/// If the path doesn't exist, copies the text to clipboard as a fallback.
pub fn open_file_or_line(line: &str) {
    debug!("Opening file or line: {line}");
    // Check if input matches "file:line:content" pattern (like grep -n output)
    if let Some((file, line_num)) = parse_file_line(line) {
        // Verify file exists before attempting to open
        if Path::new(file).exists() {
            info!("Opening file {file} at line {line_num}");
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "xdg-open".to_string());
            debug!("Using editor: {editor}");
            let mut cmd = std::process::Command::new(&editor);

            // Add line number argument for text editors (not for xdg-open)
            if editor != "xdg-open" {
                cmd.arg(format!("+{line_num}"));
            }
            cmd.arg(file);

            debug!("Spawning command: {cmd:?}");
            if let Err(e) = cmd.spawn() {
                error!("Failed to open file {file} at line {line_num}: {e}");
            } else {
                info!("Successfully opened file {file} at line {line_num}");
            }
            return;
        }
    }

    // If not a file:line pattern or file doesn't exist, try opening as plain file
    if Path::new(line).exists() {
        info!("Opening file: {line}");
        if let Err(e) = std::process::Command::new("xdg-open").arg(line).spawn() {
            error!("Failed to open file {line} with xdg-open: {e}");
        } else {
            info!("Successfully opened file: {line}");
        }
    } else {
        // Path doesn't exist - copy text to clipboard as fallback
        warn!("Path does not exist, copying to clipboard: {line}");
        copy_text(line);
        info!("Copied text to clipboard: {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_line_valid() {
        let result = parse_file_line("/path/to/file.rs:42:some content");
        assert_eq!(result, Some(("/path/to/file.rs", 42)));
    }

    #[test]
    fn test_parse_file_line_minimal() {
        let result = parse_file_line("/path/file.md:1:x");
        assert_eq!(result, Some(("/path/file.md", 1)));
    }

    #[test]
    fn test_parse_file_line_no_third_part() {
        // Requires 3 parts (file:line:content)
        let result = parse_file_line("/path/to/file.rs:42");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_file_line_zero_line() {
        let result = parse_file_line("/path/file.rs:0:content");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_file_line_non_numeric_line() {
        let result = parse_file_line("/path/file.rs:abc:content");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_file_line_empty_file() {
        let result = parse_file_line(":42:content");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_file_line_no_colons() {
        let result = parse_file_line("just-a-string");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_file_line_empty_line_num() {
        let result = parse_file_line("file::content");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_file_line_large_line() {
        let result = parse_file_line("/file:999999:content");
        assert_eq!(result, Some(("/file", 999_999)));
    }

    #[test]
    fn test_parse_file_line_empty_content() {
        let result = parse_file_line("/file:10:");
        assert_eq!(result, Some(("/file", 10)));
    }
}
