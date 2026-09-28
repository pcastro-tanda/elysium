private def Test.test
  'foo'
  rescue
  ^^^^^^ `rescue` at 3, 2 is not aligned with `private def Test.test` at 1, 0.
  'baz'
end
