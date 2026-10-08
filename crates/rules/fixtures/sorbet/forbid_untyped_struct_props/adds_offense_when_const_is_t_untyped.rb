class MyClass < T::Struct
  const :foo, T.untyped
              ^^^^^^^^^ Sorbet/ForbidUntypedStructProps: Struct props cannot be T.untyped
end
