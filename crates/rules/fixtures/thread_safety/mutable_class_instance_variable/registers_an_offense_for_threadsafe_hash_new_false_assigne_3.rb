module Test
  @var = [ThreadSafe::Hash.new { false }]
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
end