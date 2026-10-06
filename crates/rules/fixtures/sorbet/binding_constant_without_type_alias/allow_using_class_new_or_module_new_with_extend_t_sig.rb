Foo = Class.new do
  A = T.let(42, T.any(Integer, Float))
end

Foo2 = Class.new(String) do
  A = T.let(42, T.any(Integer, Float))
end

Bar = Module.new do
  A = T.let(42, T.any(Integer, Float))
end

Baz = Struct.new do
  A = T.let(42, T.any(Integer, Float))
end

Baz2 = Struct.new(:baz) do
  A = T.let(42, T.any(Integer, Float))
end

Baz3 = Struct.new(:baz, :bar, :foo) do
  A = T.let(42, T.any(Integer, Float))
end
