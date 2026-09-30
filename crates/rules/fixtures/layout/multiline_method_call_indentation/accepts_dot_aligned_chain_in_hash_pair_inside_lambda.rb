filter :season, apply: lambda { |records, values, _options|
  return records if values.blank?

  records.joins(:match)
         .where(matches: { season_id: values })
}
