class Test
  @a, @b, @c = 'foo', [2], 3
                      ^^^ Freeze mutable objects assigned to class instance variables.
               ^^^^^ Freeze mutable objects assigned to class instance variables.
end