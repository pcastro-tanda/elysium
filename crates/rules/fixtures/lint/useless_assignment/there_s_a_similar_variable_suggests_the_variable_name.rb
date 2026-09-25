def some_method
  environment = nil
  another_symbol
  enviromnent = {}
  ^^^^^^^^^^^ Useless assignment to variable - `enviromnent`. Did you mean `environment`?
  puts environment
end
