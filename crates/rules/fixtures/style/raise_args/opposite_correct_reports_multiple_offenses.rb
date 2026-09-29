if a
  raise RuntimeError, msg
elsif b
  raise Ex.new(msg)
  ^^^^^^^^^^^^^^^^^ Provide an exception class and message as arguments to `raise`.
else
  raise ArgumentError.new(msg)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Provide an exception class and message as arguments to `raise`.
end
