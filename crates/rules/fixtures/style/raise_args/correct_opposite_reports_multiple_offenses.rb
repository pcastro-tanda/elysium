if a
  raise RuntimeError, msg
  ^^^^^^^^^^^^^^^^^^^^^^^ Provide an exception object as an argument to `raise`.
elsif b
  raise Ex.new(msg)
else
  raise ArgumentError, msg
  ^^^^^^^^^^^^^^^^^^^^^^^^ Provide an exception object as an argument to `raise`.
end
