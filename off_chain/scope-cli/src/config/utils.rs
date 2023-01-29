pub mod serde_string {
    use std::{fmt::Display, str::FromStr};

    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

pub mod serde_int_map {
    use std::{collections::HashMap, fmt::Display, hash::Hash, str::FromStr};

    use nohash_hasher::{BuildNoHashHasher, IntMap};
    use serde::{de, Deserialize, Deserializer};

    // workaround this serde issue https://github.com/serde-rs/serde/issues/1183
    pub fn deserialize<'de, D, K, V>(deserializer: D) -> Result<IntMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Eq + Hash + FromStr + nohash_hasher::IsEnabled,
        K::Err: Display,
        V: Deserialize<'de>,
    {
        let string_map = <HashMap<String, V>>::deserialize(deserializer)?;
        let mut map =
            IntMap::with_capacity_and_hasher(string_map.len(), BuildNoHashHasher::default());
        for (s, v) in string_map {
            let k = K::from_str(&s).map_err(de::Error::custom)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

#[cfg(test)]
pub fn remove_whitespace(s: &str) -> String {
    s.split_whitespace().collect()
}
