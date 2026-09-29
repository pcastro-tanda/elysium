begin
  something
rescue FooException => foo
                       ^^^ Use `e` instead of `foo`.
  # do something
rescue BarException => bar
                       ^^^ Use `e` instead of `bar`.
  # do something
end
