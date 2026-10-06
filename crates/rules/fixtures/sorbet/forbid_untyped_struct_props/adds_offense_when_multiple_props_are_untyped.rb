class MyClass < T::Struct
  const :foo, T.untyped
              ^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
  const :nilable_foo, T.nilable(T.untyped)
                      ^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
  const :nested_foo, T.nilable(T.nilable(T.untyped))
                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
  prop :bar, T.untyped
             ^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
  prop :nilable_bar, T.nilable(T.untyped)
                     ^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
end
