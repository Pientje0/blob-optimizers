use serde::{Deserialize, Deserializer};

/// Windows PowerShell 5.1 collapses single-element arrays into a bare object (and empty ones into
/// `null`) when converting to JSON. Accept all three shapes as a `Vec`.
pub fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        Many(Vec<T>),
        One(T),
    }

    Ok(match Option::<OneOrMany<T>>::deserialize(deserializer)? {
        None => Vec::new(),
        Some(OneOrMany::Many(v)) => v,
        Some(OneOrMany::One(v)) => vec![v],
    })
}
