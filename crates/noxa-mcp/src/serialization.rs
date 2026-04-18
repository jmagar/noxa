use serde::Serialize;

use crate::error::NoxaMcpError;

pub fn to_pretty_json<T>(value: &T, context: &'static str) -> Result<String, NoxaMcpError>
where
    T: Serialize + ?Sized,
{
    serde_json::to_string_pretty(value)
        .map_err(|source| NoxaMcpError::Serialization { context, source })
}
