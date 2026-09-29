begin
  foo
rescue ::StandardError, BarError
  bar
rescue ::BazError, ::StandardError
  baz
end
