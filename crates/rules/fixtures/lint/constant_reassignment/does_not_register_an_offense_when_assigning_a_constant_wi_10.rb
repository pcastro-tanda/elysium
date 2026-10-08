FOO = :bar

class A
  FOO = :baz

  self.remove_const :FOO

  FOO = :quux
end
