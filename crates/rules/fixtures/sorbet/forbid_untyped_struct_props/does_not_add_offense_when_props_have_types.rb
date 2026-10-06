class MyClass < T::Struct
  const :foo, Integer
  const :nilable_foo, T.nilable(String)
  prop :bar, Date
  prop :nilable_bar, T.nilable(Float)
  const :array, T::Array[T.untyped]
  const :hash, T::Hash[T.untyped, T.untyped]
end
