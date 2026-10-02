case value
in Integer
  handle_integer
in _
  handle_other
else
^^^^ Unreachable `else` branch detected.
  handle_else
end
