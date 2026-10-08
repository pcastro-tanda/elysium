module A
  class B
    FOO = :bar
  end
end

A::B::FOO = :baz
^^^^^^^^^^^^^^^^ Constant `A::B::FOO` is already assigned in this namespace.
