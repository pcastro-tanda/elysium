module A
  class B
    FOO = :bar
  end

  class B
    FOO = :baz
    ^^^^^^^^^^ Constant `FOO` is already assigned in this namespace.
  end
end
