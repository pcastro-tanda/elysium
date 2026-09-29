def test
  some_lvar = Foo.shared_object
  some_lvar[:attr] = 1
end
