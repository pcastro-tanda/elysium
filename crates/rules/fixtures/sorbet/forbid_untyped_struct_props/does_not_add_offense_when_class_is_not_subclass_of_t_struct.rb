class MyClass < SomethingElse
  const :foo, Integer
  const :nilable_foo, T.nilable(String)
  prop :bar, Date
  prop :nilable_bar, T.nilable(Float)
end
