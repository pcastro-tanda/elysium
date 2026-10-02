case value
in Integer
  handle_integer
in String
  handle_string
else
  handle_other
end
