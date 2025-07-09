use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InvalidLatencyMatrix {
    #[error(
        "invalid line dimension: line {line} had dimension {dimension} but expected {expected}"
    )]
    InvalidLineDimension {
        line: usize,
        dimension: usize,
        expected: usize,
    },
    #[error("invalid latency value '{value}': {error}")]
    InvalidLatencyValue { value: String, error: String },
}

pub enum TimeUnit {
    Seconds,
    Milliseconds,
}

#[derive(Debug, Clone)]
pub struct LatencyMatrix {
    dimension: usize,
    latencies: Vec<Duration>,
}

impl LatencyMatrix {
    fn new(dimension: usize, latencies: Vec<Duration>) -> Self {
        assert_eq!(dimension * dimension, latencies.len());
        Self {
            dimension,
            latencies,
        }
    }

    pub fn latency(&self, row: usize, col: usize) -> Duration {
        self.latencies[self.dimension * row + col]
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn parse(content: &str, unit: TimeUnit) -> Result<Self, InvalidLatencyMatrix> {
        let mut dimension = None;
        let mut latencies = Vec::default();
        for (line_idx, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut current_dimension = 0;
            for component in line.split_whitespace() {
                current_dimension += 1;
                let component_value = match component.parse::<f64>() {
                    Ok(value) => value,
                    Err(err) => {
                        return Err(InvalidLatencyMatrix::InvalidLatencyValue {
                            value: component.to_string(),
                            error: err.to_string(),
                        });
                    }
                };

                latencies.push(Duration::from_secs_f64(match unit {
                    TimeUnit::Seconds => component_value,
                    TimeUnit::Milliseconds => component_value / 1000.0,
                }));
            }

            match dimension {
                Some(dimension) => {
                    if current_dimension != dimension {
                        return Err(InvalidLatencyMatrix::InvalidLineDimension {
                            line: line_idx,
                            dimension: current_dimension,
                            expected: dimension,
                        });
                    }
                }
                None => dimension = Some(current_dimension),
            }
        }

        Ok(Self::new(dimension.unwrap_or(0), latencies))
    }
}

impl FromStr for LatencyMatrix {
    type Err = InvalidLatencyMatrix;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s, TimeUnit::Milliseconds)
    }
}
