case value
in _ | Integer
  handle_other
in String
^^^^^^^^^ Unreachable `in` pattern branch detected.
  handle_string
end
