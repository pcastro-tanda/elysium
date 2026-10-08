module A
  class B
    FOO = :bar
  end

  B::FOO = :baz
  ^^^^^^^^^^^^^ Constant `B::FOO` is already assigned in this namespace.
end
