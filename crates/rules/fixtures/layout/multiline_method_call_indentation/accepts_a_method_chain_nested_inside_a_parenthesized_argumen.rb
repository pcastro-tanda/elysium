foo(
  key => Model.joins(
    Other
      .arel_table
      .join_sources
  )
)
