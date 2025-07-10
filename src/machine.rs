macro_rules! define_machines {
    ($(($name:ident, $idx:expr, $hostname:expr, $cpus:expr, $interface:expr)),*) => {
        #[derive(Debug)]
        pub struct UnknownMachine;

        impl std::fmt::Display for UnknownMachine {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("unknown machine")
            }
        }

        impl std::error::Error for UnknownMachine {}

        #[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
        pub enum Machine {
            $($name,)*
        }

        impl std::fmt::Display for Machine {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.hostname())
            }
        }

        impl std::str::FromStr for Machine {
            type Err = UnknownMachine;

            fn from_str(v: &str) -> Result<Self, Self::Err> {
                match v {
                    $($hostname => Ok(Self::$name),)*
                    _ => Err(UnknownMachine),
                }
            }
        }

        impl Machine {
            pub fn hostname(&self) -> &'static str {
                match self {
                    $(Self::$name => $hostname,)*
                }
            }

            pub fn index(&self) -> usize {
                match self {
                    $(Self::$name => $idx,)*
                }
            }

            pub fn from_hostname(hostname: &str) -> Option<Self> {
                match hostname {
                    $($hostname => Some(Self::$name),)*
                    _ => None
                }
            }

            pub fn from_index(index: usize) -> Option<Self> {
                match index {
                    $($idx => Some(Self::$name),)*
                    _ => None,
                }
            }

            pub fn cpus(&self) -> u32 {
                match self {
                    $(Self::$name => $cpus,)*
                }
            }

            pub fn interface(&self) -> &'static str {
                match self {
                    $(Self::$name => $interface,)*
                }
            }
        }
    };
}

// node cpu counts
// oarnodes | grep '^network_address' | cut -d' ' -f3 | sort | uniq -c

define_machines!(
    (Alakazam01, 0, "alakazam-01", 64, todo!()),
    (Alakazam02, 1, "alakazam-02", 64, todo!()),
    (Alakazam03, 2, "alakazam-03", 64, todo!()),
    (Alakazam04, 3, "alakazam-04", 64, todo!()),
    (Alakazam05, 4, "alakazam-05", 64, todo!()),
    (Alakazam06, 5, "alakazam-06", 64, todo!()),
    (Alakazam07, 6, "alakazam-07", 64, todo!()),
    (Alakazam08, 7, "alakazam-08", 64, todo!()),
    (Bulbasaur1, 8, "bulbasaur-1", 16, todo!()),
    (Bulbasaur2, 9, "bulbasaur-2", 16, todo!()),
    (Bulbasaur3, 10, "bulbasaur-3", 16, todo!()),
    (Charmander1, 11, "charmander-1", 32, "bond0"),
    (Charmander2, 12, "charmander-2", 32, "bond0"),
    (Charmander3, 13, "charmander-3", 32, "bond0"),
    (Charmander4, 14, "charmander-4", 32, "bond0"),
    (Charmander5, 15, "charmander-5", 32, "bond0"),
    (Gengar1, 16, "gengar-1", 8, "bond0"),
    (Gengar2, 17, "gengar-2", 8, "bond0"),
    (Gengar3, 18, "gengar-3", 8, "bond0"),
    (Gengar4, 19, "gengar-4", 8, "bond0"),
    (Gengar5, 20, "gengar-5", 8, "bond0"),
    (Kadabra01, 21, "kadabra-01", 64, todo!()),
    (Kadabra02, 22, "kadabra-02", 64, todo!()),
    (Kadabra03, 23, "kadabra-03", 64, todo!()),
    (Kadabra04, 24, "kadabra-04", 64, todo!()),
    (Kadabra05, 25, "kadabra-05", 64, todo!()),
    (Kadabra06, 26, "kadabra-06", 64, todo!()),
    (Kadabra07, 27, "kadabra-07", 64, todo!()),
    (Kadabra08, 28, "kadabra-08", 64, todo!()),
    (Lugia1, 29, "lugia-1", 64, "bond0"),
    (Lugia2, 30, "lugia-2", 64, "bond0"),
    (Lugia3, 31, "lugia-3", 64, "bond0"),
    (Lugia4, 32, "lugia-4", 64, "bond0"),
    (Lugia5, 33, "lugia-5", 64, "bond0"),
    (Magikarp1, 34, "magikarp-1", 16, todo!()),
    (Moltres01, 35, "moltres-01", 64, todo!()),
    (Moltres02, 36, "moltres-02", 64, todo!()),
    (Moltres03, 37, "moltres-03", 64, todo!()),
    (Moltres04, 38, "moltres-04", 64, todo!()),
    (Moltres05, 39, "moltres-05", 64, todo!()),
    (Moltres06, 40, "moltres-06", 64, todo!()),
    (Moltres07, 41, "moltres-07", 64, todo!()),
    (Moltres08, 42, "moltres-08", 64, todo!()),
    (Moltres09, 43, "moltres-09", 64, todo!()),
    (Moltres10, 44, "moltres-10", 64, todo!()),
    (Oddish1, 45, "oddish-1", 4, todo!()),
    (Psyduck1, 46, "psyduck-1", 8, todo!()),
    (Psyduck2, 47, "psyduck-2", 8, todo!()),
    (Psyduck3, 48, "psyduck-3", 8, todo!()),
    (Shelder1, 49, "shelder-1", 64, todo!()),
    (Squirtle1, 50, "squirtle-1", 24, todo!()),
    (Squirtle2, 51, "squirtle-2", 24, todo!()),
    (Squirtle3, 52, "squirtle-3", 24, todo!()),
    (Squirtle4, 53, "squirtle-4", 24, todo!()),
    (Staryu1, 54, "staryu-1", 12, todo!()),
    (Sudowoodo1, 55, "sudowoodo-1", 16, todo!()),
    (Vulpix1, 56, "vulpix-1", 112, todo!())
);
