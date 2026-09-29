def some_method(*_)
                 ^ Do not use prefix `_` for a variable that is used.
  binding
  puts _
end
