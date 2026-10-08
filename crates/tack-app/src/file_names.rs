//! Lossless native filename suffixes shared by board creation and explicit exports.
use std::path::{Path, PathBuf};

pub fn with_suffix(path: impl AsRef<Path>, suffix: &str) -> PathBuf {
    let path = path.as_ref();
    let Some(name) = path.file_name() else {
        return path.to_owned();
    };
    let bytes = name.as_encoded_bytes();
    if bytes
        .len()
        .checked_sub(suffix.len() + 1)
        .and_then(|start| bytes.get(start..))
        .is_some_and(|tail| tail[0] == b'.' && tail[1..].eq_ignore_ascii_case(suffix.as_bytes()))
    {
        return path.to_owned();
    }
    let mut name = name.to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

pub fn board(path: impl AsRef<Path>) -> PathBuf {
    with_suffix(path, "tack")
}
pub fn keymap(path: impl AsRef<Path>) -> PathBuf {
    with_suffix(path, "tackey")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suffix_appends_without_replacing_dots_and_accepts_case() {
        for (input, expected) in [
            ("myboard", "myboard.tack"),
            ("myboard.prout", "myboard.prout.tack"),
            ("my.board.v3", "my.board.v3.tack"),
            ("myboard.tack", "myboard.tack"),
            ("MYBOARD.TACK", "MYBOARD.TACK"),
            ("myboard.TaCk", "myboard.TaCk"),
            (".tack", ".tack"),
        ] {
            assert_eq!(board(input), PathBuf::from(expected));
        }
        assert_eq!(keymap("studio.json"), PathBuf::from("studio.json.tackey"));
        assert_eq!(keymap("STUDIO.TACKEY"), PathBuf::from("STUDIO.TACKEY"));
    }
    #[cfg(unix)]
    #[test]
    fn suffix_preserves_non_unicode_native_filename() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"board-\xff.other".to_vec()));
        assert_eq!(
            board(&path).as_os_str().as_bytes(),
            b"board-\xff.other.tack"
        );
    }
}
