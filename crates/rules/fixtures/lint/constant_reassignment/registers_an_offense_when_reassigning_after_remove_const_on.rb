class A
  FOO = :bar
  Other.remove_const :FOO
  FOO = :baz
  ^^^^^^^^^^ Constant `FOO` is already assigned in this namespace.
end
