if a
  raise RuntimeError, msg
else
  raise Ex.new(msg)
  ^^^^^^^^^^^^^^^^^ Provide an exception class and message as arguments to `raise`.
end
