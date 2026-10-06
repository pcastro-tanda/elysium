sig { params(a: String, b: Integer, c: String, d: Integer).void }
def foo(a, b: 1, c:, **d); end
           ^^^^ Sorbet/KeywordArgumentOrdering: Optional keyword arguments must be at the end of the parameter list.
