private def test
  'foo'
  rescue
  ^^^^^^ `rescue` at 3, 2 is not aligned with `private def test` at 1, 0.
  'baz'
end
