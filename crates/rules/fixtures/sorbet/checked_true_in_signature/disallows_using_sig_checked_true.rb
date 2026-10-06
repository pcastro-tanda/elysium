sig { params(a: Integer).void.checked(true) }
                              ^^^^^^^^^^^^^ Sorbet/CheckedTrueInSignature: Using `checked(true)` in a method signature definition is not allowed. `checked(true)` is the default behavior for modules/classes with runtime checks enabled. To enable typechecking at runtime for this module, regardless of global settings, `include(WaffleCone::RuntimeChecks)` to this module and set other methods to `checked(false)`.
def foo(a); end
