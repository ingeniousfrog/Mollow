use mollow_core::{BenchmarkSample, BenchmarkSummary};

use crate::BenchmarkError;

pub(crate) fn summarize(samples: &[BenchmarkSample]) -> Result<BenchmarkSummary, BenchmarkError> {
    if samples.is_empty() {
        return Err(BenchmarkError::new(
            "statistics",
            "at least one sample is required",
        ));
    }

    let mut rates = samples
        .iter()
        .map(|sample| sample.rate_per_second)
        .collect::<Vec<_>>();
    rates.sort_unstable();
    let median_rate = median(&rates);
    let mut deviations = rates
        .iter()
        .map(|rate| rate.abs_diff(median_rate))
        .collect::<Vec<_>>();
    deviations.sort_unstable();
    let median_absolute_deviation = median(&deviations);
    let variation_basis_points = if median_rate == 0 {
        0
    } else {
        u32::try_from(
            u128::from(median_absolute_deviation)
                .saturating_mul(10_000)
                .checked_div(u128::from(median_rate))
                .unwrap_or(0)
                .min(u128::from(u32::MAX)),
        )
        .map_err(|error| BenchmarkError::new("statistics", error.to_string()))?
    };

    Ok(BenchmarkSummary {
        median_rate_per_second: median_rate,
        median_absolute_deviation,
        minimum_rate_per_second: rates[0],
        maximum_rate_per_second: rates[rates.len() - 1],
        variation_basis_points,
    })
}

fn median(sorted: &[u64]) -> u64 {
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        sorted[middle - 1]
            .saturating_add(sorted[middle])
            .checked_div(2)
            .unwrap_or(0)
    } else {
        sorted[middle]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_uses_median_and_median_absolute_deviation() {
        let samples = [
            sample(100),
            sample(110),
            sample(90),
            sample(1_000),
            sample(105),
        ];

        let summary = summarize(&samples).expect("samples should summarize");

        assert_eq!(summary.median_rate_per_second, 105);
        assert_eq!(summary.median_absolute_deviation, 5);
        assert_eq!(summary.minimum_rate_per_second, 90);
        assert_eq!(summary.maximum_rate_per_second, 1_000);
    }

    #[test]
    fn summary_rejects_empty_samples() {
        let error = summarize(&[]).expect_err("empty samples must fail");

        assert_eq!(error.workload, "statistics");
    }

    fn sample(rate: u64) -> BenchmarkSample {
        BenchmarkSample {
            elapsed_ns: 1_000_000_000,
            work_units: rate,
            rate_per_second: rate,
        }
    }
}
