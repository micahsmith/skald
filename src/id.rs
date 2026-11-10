use rand::RngExt;

const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// `prefix` followed by eleven characters from `[a-z0-9]`, drawn from a generator seeded from the OS.
pub fn new(prefix: char) -> String {
    let mut rng = rand::rng();
    std::iter::once(prefix)
        .chain((0..11).map(|_| char::from(ALPHABET[rng.random_range(0..ALPHABET.len())])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_a_prefix_and_eleven_lowercase_alphanumerics() {
        let id = new('y');
        assert_eq!(id.len(), 12);
        assert!(id.starts_with('y'));
        assert!(
            id[1..]
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()),
            "{id}"
        );
    }

    #[test]
    fn ids_differ() {
        assert_ne!(new('y'), new('y'));
    }
}
