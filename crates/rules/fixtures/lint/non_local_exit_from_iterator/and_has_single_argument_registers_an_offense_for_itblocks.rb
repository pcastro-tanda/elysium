items.each do
  return if baz?(it)
  ^^^^^^ Non-local exit from iterator, without return value. `next`, `break`, `Array#find`, `Array#any?`, etc. is preferred.
  it.update!(foobar: true)
end
