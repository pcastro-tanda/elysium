sig { type_parameters(:x).params(a: T.type_parameter(:x)).void }
                                                     ^^ Sorbet/CapitalizedTypeParameters: Type parameters must be capitalized.
                      ^^ Sorbet/CapitalizedTypeParameters: Type parameters must be capitalized.
def foo(a)
  puts T.type_parameter(:x)
                        ^^ Sorbet/CapitalizedTypeParameters: Type parameters must be capitalized.
end

sig do
  type_parameters(:foo, :Bar, :baz)
                              ^^^^ Sorbet/CapitalizedTypeParameters: Type parameters must be capitalized.
                  ^^^^ Sorbet/CapitalizedTypeParameters: Type parameters must be capitalized.
    .params(
      a: T.type_parameter(:foo),
                          ^^^^ Sorbet/CapitalizedTypeParameters: Type parameters must be capitalized.
      b: T.type_parameter(:Bar),
      c: T.type_parameter(:baz),
                          ^^^^ Sorbet/CapitalizedTypeParameters: Type parameters must be capitalized.
    ).void
end
def foo(a, b, c); end
