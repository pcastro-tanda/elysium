class Test
  @var ||= %w(a b c)
           ^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
end