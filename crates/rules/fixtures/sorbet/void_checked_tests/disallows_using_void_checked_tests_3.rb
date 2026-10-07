sig { params(x: Integer).void.checked(:tests) }
                         ^^^^ Sorbet/VoidCheckedTests: Returning `.void` from a sig marked `.checked(:tests)` means that the method will return a different value in non-test environments (possibly with different truthiness). Either use `.returns(T.anything).checked(:tests)` to keep checking in tests, or `.void.checked(:never)` to leave it untouched.
def foo(x); end
