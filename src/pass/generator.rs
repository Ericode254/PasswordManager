use rand::Rng;

const LOWERCASE: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPERCASE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const NUMBERS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{}:,.?";
fn words() -> &'static [&'static str] {
    static WORDS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| {
        include_str!("../../assets/eff-large-wordlist.txt")
            .lines()
            .collect()
    })
}

pub fn passphrase_entropy(word_count: usize) -> f64 {
    word_count.max(6) as f64 * (words().len() as f64).log2()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorMode {
    Password,
    Passphrase,
}

pub fn generate_password(length: usize, uppercase: bool, numbers: bool, symbols: bool) -> String {
    let mut charset = LOWERCASE.to_vec();
    if uppercase {
        charset.extend_from_slice(UPPERCASE);
    }
    if numbers {
        charset.extend_from_slice(NUMBERS);
    }
    if symbols {
        charset.extend_from_slice(SYMBOLS);
    }

    let mut rng = rand::rng();
    (0..length.max(1))
        .map(|_| charset[rng.random_range(0..charset.len())] as char)
        .collect()
}

pub fn generate_passphrase(word_count: usize) -> String {
    let mut rng = rand::rng();
    let words = words();
    (0..word_count.max(6))
        .map(|_| words[rng.random_range(0..words.len())])
        .collect::<Vec<_>>()
        .join("-")
}

pub fn strength(password: &str) -> (u8, f64) {
    if password.is_empty() {
        return (0, 0.0);
    }

    let result = zxcvbn::zxcvbn(password, &[]);
    let score = result.score() as u8;
    let entropy_bits = result.guesses_log10() * std::f64::consts::LOG2_10;
    (score, entropy_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_password_has_requested_length() {
        let password = generate_password(24, true, true, true);
        assert_eq!(password.len(), 24);
    }

    #[test]
    fn generated_passphrase_has_requested_words() {
        let phrase = generate_passphrase(6);
        assert_eq!(phrase.split('-').count(), 6);
    }
}

#[cfg(test)]
mod wordlist_tests {
    use super::*;
    #[test]
    fn wordlist_is_unique_and_minimum_phrase_has_sufficient_entropy() {
        let unique: std::collections::HashSet<_> = words().iter().collect();
        assert_eq!(unique.len(), 7776);
        assert_eq!(unique.len(), words().len());
        let phrase = generate_passphrase(1);
        let parts: Vec<_> = phrase.split('-').collect();
        assert_eq!(parts.len(), 6);
        assert!(parts.iter().all(|word| words().contains(word)));
        assert!(passphrase_entropy(6) > 77.0);
    }
}
