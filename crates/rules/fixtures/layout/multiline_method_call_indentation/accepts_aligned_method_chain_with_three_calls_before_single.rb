users
  .dup.compact.sort_by { _1.name }
  .first(10)
