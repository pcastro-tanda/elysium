module Test
  @a, @b, @c = [1, { a: 1 }, [3].freeze]
                   ^^^^^^^^ Freeze mutable objects assigned to class instance variables.
end