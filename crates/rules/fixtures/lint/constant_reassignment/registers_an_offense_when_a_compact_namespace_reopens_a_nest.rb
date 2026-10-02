module A
  module B
    FOO = :bar
  end
end

module A::B
  FOO = :baz
  ^^^^^^^^^^ Constant `FOO` is already assigned in this namespace.
end
