use std::sync::Arc;

use eyre::{Context as _, Result};
use futures::{StreamExt as _, stream::FuturesUnordered};
use tokio::sync::Semaphore;

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
    (Alakazam01, 0, "alakazam-01", 64, "bond0"),
    (Alakazam02, 1, "alakazam-02", 64, "bond0"),
    (Alakazam03, 2, "alakazam-03", 64, "bond0"),
    (Alakazam04, 3, "alakazam-04", 64, "bond0"),
    (Alakazam05, 4, "alakazam-05", 64, "bond0"),
    (Alakazam06, 5, "alakazam-06", 64, "bond0"),
    (Alakazam07, 6, "alakazam-07", 64, "bond0"),
    (Alakazam08, 7, "alakazam-08", 64, "bond0"),
    (Bulbasaur1, 8, "bulbasaur-1", 16, "bond0"),
    (Bulbasaur2, 9, "bulbasaur-2", 16, "bond0"),
    (Bulbasaur3, 10, "bulbasaur-3", 16, "bond0"),
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
    (Kadabra01, 21, "kadabra-01", 64, "bond0"),
    (Kadabra02, 22, "kadabra-02", 64, "bond0"),
    (Kadabra03, 23, "kadabra-03", 64, "bond0"),
    (Kadabra04, 24, "kadabra-04", 64, "bond0"),
    (Kadabra05, 25, "kadabra-05", 64, "bond0"),
    (Kadabra06, 26, "kadabra-06", 64, "bond0"),
    (Kadabra07, 27, "kadabra-07", 64, "bond0"),
    (Kadabra08, 28, "kadabra-08", 64, "bond0"),
    (Lugia1, 29, "lugia-1", 64, "bond0"),
    (Lugia2, 30, "lugia-2", 64, "bond0"),
    (Lugia3, 31, "lugia-3", 64, "bond0"),
    (Lugia4, 32, "lugia-4", 64, "bond0"),
    (Lugia5, 33, "lugia-5", 64, "bond0"),
    (Magikarp1, 34, "magikarp-1", 16, todo!()),
    (Moltres01, 35, "moltres-01", 64, "bond0"),
    (Moltres02, 36, "moltres-02", 64, "bond0"),
    (Moltres03, 37, "moltres-03", 64, "bond0"),
    (Moltres04, 38, "moltres-04", 64, "bond0"),
    (Moltres05, 39, "moltres-05", 64, "bond0"),
    (Moltres06, 40, "moltres-06", 64, "bond0"),
    (Moltres07, 41, "moltres-07", 64, "bond0"),
    (Moltres08, 42, "moltres-08", 64, "bond0"),
    (Moltres09, 43, "moltres-09", 64, "bond0"),
    (Moltres10, 44, "moltres-10", 64, "bond0"),
    (Oddish1, 45, "oddish-1", 4, "bond0"),
    (Psyduck1, 46, "psyduck-1", 8, "bond0"),
    (Psyduck2, 47, "psyduck-2", 8, "bond0"),
    (Psyduck3, 48, "psyduck-3", 8, "bond0"),
    (Shelder1, 49, "shelder-1", 64, todo!()),
    (Squirtle1, 50, "squirtle-1", 24, "bond0"),
    (Squirtle2, 51, "squirtle-2", 24, "bond0"),
    (Squirtle3, 52, "squirtle-3", 24, "bond0"),
    (Squirtle4, 53, "squirtle-4", 24, "bond0"),
    (Staryu1, 54, "staryu-1", 12, todo!()),
    (Sudowoodo1, 55, "sudowoodo-1", 16, todo!()),
    (Vulpix1, 56, "vulpix-1", 112, todo!()),
    (Snorlax01, 57, "snorlax-01", 64, "bond0"),
    (Snorlax02, 58, "snorlax-02", 64, "bond0"),
    (Snorlax03, 59, "snorlax-03", 64, "bond0")
);

pub async fn for_each<F, FUT, RET>(
    machines: impl IntoIterator<Item = &Machine>,
    f: F,
) -> Result<Vec<(Machine, RET)>>
where
    F: Fn(Machine) -> FUT,
    RET: Send + 'static,
    FUT: std::future::Future<Output = Result<RET>>,
{
    let limit = match std::env::var("OAR_P2P_CONCURRENCY_LIMIT") {
        Ok(value) => {
            tracing::trace!("parsing concurrency limit value '{value}'");
            let limit = value
                .parse()
                .expect("invalid value for OAR_P2P_CONCURRENCY_LIMIT");
            tracing::debug!("using concurrency limit = {limit}");
            limit
        }
        Err(_) => 0,
    };
    for_each_with_limit(machines, limit, f).await
}

pub async fn for_each_with_limit<F, FUT, RET>(
    machines: impl IntoIterator<Item = &Machine>,
    limit: usize,
    f: F,
) -> Result<Vec<(Machine, RET)>>
where
    F: Fn(Machine) -> FUT,
    RET: Send + 'static,
    FUT: std::future::Future<Output = Result<RET>>,
{
    let sem = Arc::new(Semaphore::new(if limit == 0 {
        Semaphore::MAX_PERMITS
    } else {
        limit
    }));
    let mut futures = FuturesUnordered::new();

    for &machine in machines {
        let fut = f(machine);
        let sem = sem.clone();
        let fut = async move {
            let _permit = sem.acquire().await.unwrap();
            (machine, fut.await)
        };
        futures.push(fut);
    }

    let mut results = Vec::default();
    while let Some((machine, result)) = futures.next().await {
        match result {
            Ok(value) => results.push((machine, value)),
            Err(err) => {
                return Err(err).with_context(|| format!("running task on machine {machine}"));
            }
        }
    }
    Ok(results)
}
