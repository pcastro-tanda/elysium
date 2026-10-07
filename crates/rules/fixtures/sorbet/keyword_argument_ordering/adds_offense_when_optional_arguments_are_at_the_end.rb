sig { params(a: Integer, b: String, blk: Proc).void }
def foo(a: 1, b:, &blk); end
        ^^^^ Sorbet/KeywordArgumentOrdering: Optional keyword arguments must be at the end of the parameter list.
