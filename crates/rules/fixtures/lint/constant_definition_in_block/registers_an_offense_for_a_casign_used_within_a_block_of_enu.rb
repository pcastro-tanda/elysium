class TestEnum < T::Enum
  enums do
    Foo = new("foo")
    ^^^^^^^^^^^^^^^^ Do not define constants this way within a block.
  end
end
