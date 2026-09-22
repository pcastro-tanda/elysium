def f
  return value if items.filter_map { |item| item.do_something if item.something? }
                                                              ^^ Modifier form of `if` makes the line too long.
               ^^ Modifier form of `if` makes the line too long.
end
