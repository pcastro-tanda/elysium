sig { type_parameters(:X).params(a: T.type_parameter(:X)).void }
def foo(a); end

sig do
  type_parameters(:Foo, :Bar, :Baz)
    .params(
      a: T.type_parameter(:Foo),
      b: T.type_parameter(:Bar),
      c: T.type_parameter(:Baz),
    ).void
end
def foo(a, b, c); end
