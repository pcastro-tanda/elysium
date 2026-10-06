class MyClass < T::Struct
  const :foo, T.nilable(T.untyped)
              ^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
end
