module Test
  @a = [1].freeze
  @b = [2].freeze
  @c = [3].freeze
  @var = @a + @b + @c
         ^^^^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
end