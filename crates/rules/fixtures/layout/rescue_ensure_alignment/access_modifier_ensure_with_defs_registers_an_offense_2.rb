private_class_method def Test.test
  'foo'
  ensure
  ^^^^^^ `ensure` at 3, 2 is not aligned with `private_class_method def Test.test` at 1, 0.
  'baz'
end
