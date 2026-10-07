class MyClass < T::Struct
  prop :foo, T.untyped
             ^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
end
