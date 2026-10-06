class Test
  @var ||= [Concurrent::ThreadSafe::Util::Adder.new]
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
end