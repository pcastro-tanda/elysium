class MyClass < T::Struct
  prop :foo, T.nilable(T.untyped), immutable: true
             ^^^^^^^^^^^^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
end
