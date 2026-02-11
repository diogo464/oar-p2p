#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressAllocationPolicy {
    PerCpu(u32),
    PerMachine(u32),
    Total(u32),
}

#[derive(Debug)]
pub struct InvalidAddressAllocationPolicy(String);

impl std::fmt::Display for InvalidAddressAllocationPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid address allocation policy: ")?;
        f.write_str(&self.0)
    }
}

impl std::error::Error for InvalidAddressAllocationPolicy {}

impl From<std::num::ParseIntError> for InvalidAddressAllocationPolicy {
    fn from(value: std::num::ParseIntError) -> Self {
        Self(value.to_string())
    }
}

impl std::str::FromStr for AddressAllocationPolicy {
    type Err = InvalidAddressAllocationPolicy;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(n) = s.strip_suffix("/cpu") {
            Ok(Self::PerCpu(n.parse()?))
        } else if let Some(n) = s.strip_suffix("/machine") {
            Ok(Self::PerMachine(n.parse()?))
        } else {
            Ok(Self::Total(s.parse()?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_per_cpu_parsing() {
        assert_eq!(
            AddressAllocationPolicy::from_str("10/cpu").unwrap(),
            AddressAllocationPolicy::PerCpu(10)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("1/cpu").unwrap(),
            AddressAllocationPolicy::PerCpu(1)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("1000/cpu").unwrap(),
            AddressAllocationPolicy::PerCpu(1000)
        );
    }

    #[test]
    fn test_per_machine_parsing() {
        assert_eq!(
            AddressAllocationPolicy::from_str("20/machine").unwrap(),
            AddressAllocationPolicy::PerMachine(20)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("1/machine").unwrap(),
            AddressAllocationPolicy::PerMachine(1)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("500/machine").unwrap(),
            AddressAllocationPolicy::PerMachine(500)
        );
    }

    #[test]
    fn test_total_parsing() {
        assert_eq!(
            AddressAllocationPolicy::from_str("100").unwrap(),
            AddressAllocationPolicy::Total(100)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("1").unwrap(),
            AddressAllocationPolicy::Total(1)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("9999").unwrap(),
            AddressAllocationPolicy::Total(9999)
        );
    }

    #[test]
    fn test_invalid_number_formats() {
        assert!(AddressAllocationPolicy::from_str("-5/cpu").is_err());
        assert!(AddressAllocationPolicy::from_str("abc/cpu").is_err());
        assert!(AddressAllocationPolicy::from_str("10.5/machine").is_err());
        assert!(AddressAllocationPolicy::from_str("xyz").is_err());
        assert!(AddressAllocationPolicy::from_str("").is_err());
    }

    #[test]
    fn test_invalid_suffixes() {
        assert!(AddressAllocationPolicy::from_str("10/node").is_err());
        assert!(AddressAllocationPolicy::from_str("10/core").is_err());
        assert!(AddressAllocationPolicy::from_str("10/").is_err());
    }

    #[test]
    fn test_zero_values() {
        assert_eq!(
            AddressAllocationPolicy::from_str("0/cpu").unwrap(),
            AddressAllocationPolicy::PerCpu(0)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("0/machine").unwrap(),
            AddressAllocationPolicy::PerMachine(0)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("0").unwrap(),
            AddressAllocationPolicy::Total(0)
        );
    }

    #[test]
    fn test_large_numbers() {
        assert_eq!(
            AddressAllocationPolicy::from_str("4294967295/cpu").unwrap(),
            AddressAllocationPolicy::PerCpu(u32::MAX)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("4294967295/machine").unwrap(),
            AddressAllocationPolicy::PerMachine(u32::MAX)
        );
        assert_eq!(
            AddressAllocationPolicy::from_str("4294967295").unwrap(),
            AddressAllocationPolicy::Total(u32::MAX)
        );
    }

    #[test]
    fn test_overflow() {
        assert!(AddressAllocationPolicy::from_str("4294967296/cpu").is_err());
        assert!(AddressAllocationPolicy::from_str("9999999999999/machine").is_err());
        assert!(AddressAllocationPolicy::from_str("18446744073709551616").is_err());
    }

    #[test]
    fn test_whitespace_handling() {
        assert!(AddressAllocationPolicy::from_str(" 10/cpu").is_err());
        assert!(AddressAllocationPolicy::from_str("10/cpu ").is_err());
        assert!(AddressAllocationPolicy::from_str("10 /cpu").is_err());
        assert!(AddressAllocationPolicy::from_str("10/ cpu").is_err());
    }
}
