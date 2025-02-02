#[cfg(target_os = "windows")]
pub fn is_valid_path(path: &str) -> bool {
    let invalid_chars = ['<', '>', ':', '"', '|', '?', '*'];
    !path.chars().any(|c| invalid_chars.contains(&c)) && !path.trim().is_empty()
}

#[cfg(any(unix, target_os = "macos"))]
pub fn is_valid_path(path: &str) -> bool {
    !path.contains('\0') && !path.split('/').any(|part| part.is_empty())
}

pub struct SendT<T>(pub T);

unsafe impl<T> Send for SendT<T> {}
unsafe impl<T> Sync for SendT<T> {}
