timeseries.push(
  {
    value: timeseries_snapshots.pluck(:membership_gross_value)
                               .compact_sum
                               .to_f,
  },
)
