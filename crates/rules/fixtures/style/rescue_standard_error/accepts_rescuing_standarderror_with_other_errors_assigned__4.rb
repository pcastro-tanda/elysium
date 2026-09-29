def foobar
  foo
rescue StandardError, BarError => e
  bar
rescue BazError, StandardError => e
  baz
end
