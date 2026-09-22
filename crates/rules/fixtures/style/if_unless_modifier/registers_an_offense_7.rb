def f
  puts '                                                         ' if condition # rubocop:disable Style/For
                                                                   ^^ Modifier form of `if` makes the line too long.
end
