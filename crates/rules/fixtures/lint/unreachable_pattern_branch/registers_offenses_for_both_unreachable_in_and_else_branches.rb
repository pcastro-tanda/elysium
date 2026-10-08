case value
in x
  handle_other
in Integer
^^^^^^^^^^ Unreachable `in` pattern branch detected.
  handle_integer
else
^^^^ Unreachable `else` branch detected.
  handle_else
end
