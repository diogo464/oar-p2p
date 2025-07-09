macro_rules! define_machines {
    ($(($name:ident, $idx:expr, $hostname:expr, $interface:expr)),*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Machine {
            $($name,)*
        }

        impl std::fmt::Display for Machine {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.hostname())
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

            pub fn interface(&self) -> &'static str {
                match self {
                    $(Self::$name => $interface,)*
                }
            }
        }
    };
}

define_machines!(
    (Alakazam01, 0, "alakazam-01", todo!()),
    (Alakazam02, 1, "alakazam-02", todo!()),
    (Alakazam03, 2, "alakazam-03", todo!()),
    (Alakazam04, 3, "alakazam-04", todo!()),
    (Alakazam05, 4, "alakazam-05", todo!()),
    (Alakazam06, 5, "alakazam-06", todo!()),
    (Alakazam07, 6, "alakazam-07", todo!()),
    (Alakazam08, 7, "alakazam-08", todo!()),
    (Bulbasaur1, 8, "bulbasaur-1", todo!()),
    (Bulbasaur2, 9, "bulbasaur-2", todo!()),
    (Bulbasaur3, 10, "bulbasaur-3", todo!()),
    (Charmander1, 11, "charmander-1", "bond0"),
    (Charmander2, 12, "charmander-2", "bond0"),
    (Charmander3, 13, "charmander-3", "bond0"),
    (Charmander4, 14, "charmander-4", "bond0"),
    (Charmander5, 15, "charmander-5", "bond0"),
    (Gengar1, 16, "gengar-1", "bond0"),
    (Gengar2, 17, "gengar-2", "bond0"),
    (Gengar3, 18, "gengar-3", "bond0"),
    (Gengar4, 19, "gengar-4", "bond0"),
    (Gengar5, 20, "gengar-5", "bond0"),
    (Kadabra01, 21, "kadabra-01", todo!()),
    (Kadabra02, 22, "kadabra-02", todo!()),
    (Kadabra03, 23, "kadabra-03", todo!()),
    (Kadabra04, 24, "kadabra-04", todo!()),
    (Kadabra05, 25, "kadabra-05", todo!()),
    (Kadabra06, 26, "kadabra-06", todo!()),
    (Kadabra07, 27, "kadabra-07", todo!()),
    (Kadabra08, 28, "kadabra-08", todo!()),
    (Lugia1, 29, "lugia-1", "bond0"),
    (Lugia2, 30, "lugia-2", "bond0"),
    (Lugia3, 31, "lugia-3", "bond0"),
    (Lugia4, 32, "lugia-4", "bond0"),
    (Lugia5, 33, "lugia-5", "bond0"),
    (Magikarp1, 34, "magikarp-1", todo!()),
    (Moltres01, 35, "moltres-01", todo!()),
    (Moltres02, 36, "moltres-02", todo!()),
    (Moltres03, 37, "moltres-03", todo!()),
    (Moltres04, 38, "moltres-04", todo!()),
    (Moltres05, 39, "moltres-05", todo!()),
    (Moltres06, 40, "moltres-06", todo!()),
    (Moltres07, 41, "moltres-07", todo!()),
    (Moltres08, 42, "moltres-08", todo!()),
    (Moltres09, 43, "moltres-09", todo!()),
    (Moltres10, 44, "moltres-10", todo!()),
    (Oddish1, 45, "oddish-1", todo!()),
    (Psyduck1, 46, "psyduck-1", todo!()),
    (Psyduck2, 47, "psyduck-2", todo!()),
    (Psyduck3, 48, "psyduck-3", todo!()),
    (Shelder1, 49, "shelder-1", todo!()),
    (Squirtle1, 50, "squirtle-1", todo!()),
    (Squirtle2, 51, "squirtle-2", todo!()),
    (Squirtle3, 52, "squirtle-3", todo!()),
    (Squirtle4, 53, "squirtle-4", todo!()),
    (Staryu1, 54, "staryu-1", todo!()),
    (Sudowoodo1, 55, "sudowoodo-1", todo!()),
    (Vulpix1, 56, "vulpix-1", todo!())
);
