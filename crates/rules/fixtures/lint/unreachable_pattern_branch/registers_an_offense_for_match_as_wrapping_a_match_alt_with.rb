case value
in (_ | Integer) => y
  handle_other
in String
^^^^^^^^^ Unreachable `in` pattern branch detected.
  handle_string
end
