begin
  something
rescue FooException => foo
                       ^^^ Use `exception` instead of `foo`.
  # do something
rescue BarException => bar
                       ^^^ Use `exception` instead of `bar`.
  # do something
end
