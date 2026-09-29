if a
  raise RuntimeError, msg
  ^^^^^^^^^^^^^^^^^^^^^^^ Provide an exception object as an argument to `raise`.
else
  raise Ex.new(msg)
end
