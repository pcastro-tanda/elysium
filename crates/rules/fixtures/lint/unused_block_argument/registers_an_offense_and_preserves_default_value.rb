obj.method do |foo, bar = baz|
                    ^^^ Unused block argument - `bar`. If it's necessary, use `_` or `_bar` as an argument name to indicate that it won't be used.
  stuff(foo)
end
