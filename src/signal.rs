use std::{str::FromStr, time::Duration};

const SIGNAL_MIN_LEN: usize = 1;
const SIGNAL_MAX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Signal(String);

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Signal {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub struct InvalidSignal(String);

impl std::fmt::Display for InvalidSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid signal '{}'. a signal must be composed of alphanumeric, '-' or '_' and be between 1 and 64 characters long",
            self.0
        )
    }
}

impl std::error::Error for InvalidSignal {}

impl FromStr for Signal {
    type Err = InvalidSignal;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < SIGNAL_MIN_LEN
            || s.len() > SIGNAL_MAX_LEN
            || !s.chars().all(is_valid_signal_char)
        {
            Err(InvalidSignal(s.to_string()))
        } else {
            Ok(Self(s.to_string()))
        }
    }
}

fn is_valid_signal_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalSpec {
    pub signal: Signal,
    pub delay: Duration,
}

#[derive(Debug)]
pub struct InvalidSignalSpec(String);

impl std::fmt::Display for InvalidSignalSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid signal spec '{}'. signal spec must be in format <signal>:<seconds>",
            self.0
        )
    }
}

impl std::error::Error for InvalidSignalSpec {}

impl FromStr for SignalSpec {
    type Err = InvalidSignalSpec;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_fn = || InvalidSignalSpec(s.to_string());
        let (lhs, rhs) = s.split_once(':').ok_or_else(err_fn)?;
        let signal = lhs.parse().ok().ok_or_else(err_fn)?;
        let delay = Duration::from_secs(rhs.parse().ok().ok_or_else(err_fn)?);
        Ok(Self { signal, delay })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_signal_valid() {
        let max_length_signal = "a".repeat(64);
        let valid_signals = vec![
            "a",
            "test",
            "test-signal",
            "test_signal",
            "123",
            "abc123",
            &max_length_signal, // max length
        ];

        for signal_str in valid_signals {
            let signal: Result<Signal, _> = signal_str.parse();
            assert!(signal.is_ok(), "Signal '{}' should be valid", signal_str);
            assert_eq!(signal.unwrap().as_str(), signal_str);
        }
    }

    #[test]
    fn test_signal_invalid_empty() {
        let signal: Result<Signal, _> = "".parse();
        assert!(signal.is_err());
    }

    #[test]
    fn test_signal_invalid_too_long() {
        let long_signal = "a".repeat(65); // max + 1
        let signal: Result<Signal, _> = long_signal.parse();
        assert!(signal.is_err());
    }

    #[test]
    fn test_signal_invalid_characters() {
        let invalid_signals = vec![
            "test signal",  // space
            "test@signal",  // @
            "test.signal",  // .
            "test/signal",  // /
            "test\\signal", // \
            "test!signal",  // !
            "test#signal",  // #
            "test$signal",  // $
        ];

        for signal_str in invalid_signals {
            let signal: Result<Signal, _> = signal_str.parse();
            assert!(signal.is_err(), "Signal '{}' should be invalid", signal_str);
        }
    }

    #[test]
    fn test_signal_clone_and_equality() {
        let signal1: Signal = "test-signal".parse().unwrap();
        let signal2 = signal1.clone();
        assert_eq!(signal1, signal2);
    }

    #[test]
    fn test_signal_debug() {
        let signal: Signal = "test".parse().unwrap();
        let debug_str = format!("{:?}", signal);
        assert!(debug_str.contains("Signal"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_signal_spec_valid() {
        let valid_specs = vec![
            ("test:5", "test", 5),
            ("signal-name:10", "signal-name", 10),
            ("a:0", "a", 0),
            ("long_signal_name:3600", "long_signal_name", 3600),
        ];

        for (spec_str, expected_signal, expected_seconds) in valid_specs {
            let spec: Result<SignalSpec, _> = spec_str.parse();
            assert!(spec.is_ok(), "SignalSpec '{}' should be valid", spec_str);

            let spec = spec.unwrap();
            assert_eq!(spec.signal.as_str(), expected_signal);
            assert_eq!(spec.delay, Duration::from_secs(expected_seconds));
        }
    }

    #[test]
    fn test_signal_spec_invalid_no_colon() {
        let spec: Result<SignalSpec, _> = "test5".parse();
        assert!(spec.is_err());
    }

    #[test]
    fn test_signal_spec_invalid_signal() {
        let spec: Result<SignalSpec, _> = "bad@signal:5".parse();
        assert!(spec.is_err());
    }

    #[test]
    fn test_signal_spec_invalid_delay() {
        let invalid_delays = vec![
            "test:abc", // non-numeric
            "test:-5",  // negative
            "test:5.5", // float
            "test:",    // empty delay
        ];

        for spec_str in invalid_delays {
            let spec: Result<SignalSpec, _> = spec_str.parse();
            assert!(spec.is_err(), "SignalSpec '{}' should be invalid", spec_str);
        }
    }

    #[test]
    fn test_signal_spec_clone_and_equality() {
        let spec1: SignalSpec = "test:5".parse().unwrap();
        let spec2 = spec1.clone();
        assert_eq!(spec1, spec2);
    }

    #[test]
    fn test_signal_spec_debug() {
        let spec: SignalSpec = "test:5".parse().unwrap();
        let debug_str = format!("{:?}", spec);
        assert!(debug_str.contains("SignalSpec"));
    }

    #[test]
    fn test_signal_boundary_lengths() {
        // Test minimum length (1 character)
        let min_signal: Signal = "a".parse().unwrap();
        assert_eq!(min_signal.as_str(), "a");

        // Test maximum length (64 characters)
        let max_signal_str = "a".repeat(64);
        let max_signal: Signal = max_signal_str.parse().unwrap();
        assert_eq!(max_signal.as_str(), max_signal_str);
    }

    #[test]
    fn test_is_valid_signal_char() {
        // Valid characters
        assert!(is_valid_signal_char('a'));
        assert!(is_valid_signal_char('Z'));
        assert!(is_valid_signal_char('0'));
        assert!(is_valid_signal_char('9'));
        assert!(is_valid_signal_char('_'));
        assert!(is_valid_signal_char('-'));

        // Invalid characters
        assert!(!is_valid_signal_char(' '));
        assert!(!is_valid_signal_char('@'));
        assert!(!is_valid_signal_char('.'));
        assert!(!is_valid_signal_char('/'));
        assert!(!is_valid_signal_char('!'));
    }

    #[test]
    fn test_signal_spec_zero_delay() {
        let spec: SignalSpec = "test:0".parse().unwrap();
        assert_eq!(spec.delay, Duration::from_secs(0));
    }

    #[test]
    fn test_signal_spec_large_delay() {
        let spec: SignalSpec = "test:86400".parse().unwrap(); // 24 hours
        assert_eq!(spec.delay, Duration::from_secs(86400));
    }
}
