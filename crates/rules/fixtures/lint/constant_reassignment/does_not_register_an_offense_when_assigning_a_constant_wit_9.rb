FOO = :bar

class A
  FOO = :baz

  remove_const 'FOO'

  FOO = :quux
end
