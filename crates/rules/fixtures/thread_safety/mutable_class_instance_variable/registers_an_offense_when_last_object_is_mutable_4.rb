module Test
  @a, _, @c = 1, [2].freeze, 'foo'
                             ^^^^^ Freeze mutable objects assigned to class instance variables.
end