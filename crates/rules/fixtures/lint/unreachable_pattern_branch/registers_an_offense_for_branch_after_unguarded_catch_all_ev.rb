case value
in x if x.positive?
  handle_positive
in y
  handle_other
in Integer
^^^^^^^^^^ Unreachable `in` pattern branch detected.
  handle_integer
end
