{}.reject { |x| x.is_a?(Foo) }
{ foo: :bar }.reject { |x| x.is_a?(Foo) }
