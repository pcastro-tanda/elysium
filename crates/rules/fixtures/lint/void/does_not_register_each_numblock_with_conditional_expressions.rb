enumerator_as_filter.each do
  puts _1

  # The `filter` method is used to filter for matches with `42`.
  # In this case, it's not void.
  _1 == 42
end
