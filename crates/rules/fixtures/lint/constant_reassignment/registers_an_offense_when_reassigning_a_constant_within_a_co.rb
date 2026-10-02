class A
  FOO = :bar
  FOO = :baz
  ^^^^^^^^^^ Constant `FOO` is already assigned in this namespace.
end unless defined?(A)
