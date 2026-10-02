case value
in x if x.positive?
  handle_positive
in Integer
  handle_integer
end
