class MyEnum < T::Enum
  class Foo
    prepend Comparable
  end
end
