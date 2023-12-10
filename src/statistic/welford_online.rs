/// Calculates the next mean.
pub fn calculate_mean<T>(mut prev_mean: T, next_value: T, count: T) -> T
where
    T: Copy + std::ops::Sub<Output = T> + std::ops::Div<Output = T> + std::ops::AddAssign,
{
    prev_mean += (next_value - prev_mean) / count;
    prev_mean
}

/// Calculates the next Welford Online recurrence relation M.
pub fn calculate_recurrence_relation_m(
    prev_m: f64,
    prev_mean: f64,
    new_value: f64,
    new_mean: f64,
) -> f64 {
    prev_m + ((new_value - prev_mean) * (new_value - new_mean))
}

/// Calculates the next unbiased 'Sample' Variance using Bessel's correction (count - 1), and the
/// Welford Online recurrence relation M.
pub fn calculate_sample_variance(recurrence_relation_m: f64, count: u64) -> f64 {
    match count < 2 {
        true => 0.0,
        false => recurrence_relation_m / (count as f64 - 1.0),
    }
}

/// Calculates the next biased 'Population' Variance using the Welford Online recurrence relation M.
pub fn calculate_population_variance(recurrence_relation_m: f64, count: u64) -> f64 {
    match count < 1 {
        true => 0.0,
        false => recurrence_relation_m / count as f64,
    }
}
