case value
in Integer
  handle_integer
in x
  handle_other
in String
^^^^^^^^^ Unreachable `in` pattern branch detected.
  handle_string
in Symbol
^^^^^^^^^ Unreachable `in` pattern branch detected.
  handle_symbol
end
