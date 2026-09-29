def baz
  foo
rescue BarError => e
  bar
end
