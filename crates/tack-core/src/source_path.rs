use crate::{MAX_SOURCE_PATH_BYTES, ModelError};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathPlatform {
    Unix,
    Windows,
}

/// Lossless bytes: Unix native bytes or Windows UTF-16LE, never lossy UTF-8.
/// Relative descriptors resolve against the board's parent directory, outside core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedPath {
    platform: PathPlatform,
    absolute: bool,
    bytes: Vec<u8>,
}
impl LinkedPath {
    pub fn encoded(
        platform: PathPlatform,
        absolute: bool,
        bytes: &[u8],
    ) -> Result<Self, ModelError> {
        let valid = match platform {
            PathPlatform::Unix => !bytes.contains(&0),
            PathPlatform::Windows => {
                bytes.len().is_multiple_of(2) && !bytes.chunks_exact(2).any(|c| c == [0, 0])
            }
        };
        if bytes.is_empty() || bytes.len() > MAX_SOURCE_PATH_BYTES || !valid {
            return Err(ModelError::InvalidSourcePath);
        }
        let descriptor = Self {
            platform,
            absolute,
            bytes: bytes.to_vec(),
        };
        if let Some(path) = descriptor.to_native()
            && path.is_absolute() != absolute
        {
            return Err(ModelError::InvalidSourcePath);
        }
        Ok(descriptor)
    }
    pub fn native(path: &Path) -> Result<Self, ModelError> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Self::encoded(
                PathPlatform::Unix,
                path.is_absolute(),
                path.as_os_str().as_bytes(),
            )
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let bytes: Vec<u8> = path
                .as_os_str()
                .encode_wide()
                .flat_map(u16::to_le_bytes)
                .collect();
            Self::encoded(PathPlatform::Windows, path.is_absolute(), &bytes)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            Err(ModelError::InvalidSourcePath)
        }
    }
    pub fn platform(&self) -> PathPlatform {
        self.platform
    }
    pub fn is_absolute(&self) -> bool {
        self.absolute
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Foreign descriptors remain preservable but are never resolved as local text.
    pub fn to_native(&self) -> Option<PathBuf> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            if self.platform == PathPlatform::Unix {
                return Some(std::ffi::OsString::from_vec(self.bytes.clone()).into());
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            if self.platform == PathPlatform::Windows {
                let wide: Vec<u16> = self
                    .bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let path: PathBuf = std::ffi::OsString::from_wide(&wide).into();
                // Drive/root-relative Windows paths do not have board-parent semantics.
                // Keep their bytes, but require explicit relink instead of ambient drive state.
                if !self.absolute
                    && (path.has_root()
                        || path
                            .components()
                            .any(|c| matches!(c, std::path::Component::Prefix(_))))
                {
                    return None;
                }
                return Some(path);
            }
        }
        None
    }
}
