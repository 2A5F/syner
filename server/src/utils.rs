use std::fmt;
use std::str::FromStr;

use headers::Header;
use headers::HeaderValue;
use warp::Filter;
use warp::Rejection;

#[cfg(target_os = "windows")]
pub fn is_valid_path(path: &str) -> bool {
    let invalid_chars = ['<', '>', ':', '"', '|', '?', '*'];
    !path.chars().any(|c| invalid_chars.contains(&c)) && !path.trim().is_empty()
}

#[cfg(any(unix, target_os = "macos"))]
pub fn is_valid_path(path: &str) -> bool {
    !path.contains('\0') && !path.split('/').any(|part| part.is_empty())
}

pub fn optional_header<T: Header + Send + 'static>(
) -> impl Filter<Extract = (Option<T>,), Error = Rejection> + Clone {
    warp::header::optional(T::name().as_str()).and_then(
        |value: Option<HeaderFromStr<T>>| async move {
            if let Some(value) = value {
                let value = value.value;
                let r: Result<_, Rejection> = Ok(Some(value));
                r
            } else {
                Ok(None)
            }
        },
    )
}

struct HeaderFromStr<T> {
    value: T,
}

impl<T: Header> FromStr for HeaderFromStr<T> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = HeaderValue::from_str(&s)?;
        let value = T::decode(&mut [value].iter())?;
        Ok(Self { value })
    }
}

#[derive(Debug)]
pub struct InvalidHeader {
    name: &'static str,
}

impl warp::reject::Reject for InvalidHeader {}

impl fmt::Display for InvalidHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid request header {:?}", self.name)
    }
}
