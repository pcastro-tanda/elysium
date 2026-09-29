foo = [{foo: :bar}].find { |h|
  h.key?(:foo)
}[:foo]
