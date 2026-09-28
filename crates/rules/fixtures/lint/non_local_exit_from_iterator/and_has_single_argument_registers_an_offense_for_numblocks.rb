items.each do
  return if baz?(_1)
  ^^^^^^ Non-local exit from iterator, without return value. `next`, `break`, `Array#find`, `Array#any?`, etc. is preferred.
  _1.update!(foobar: true)
end
