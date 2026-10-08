module MyModule
  extend(T::Sig)
  ^^^^^^^^^^^^^^ Sorbet/ForbidExtendTSigHelpersInShims: Extending T::Sig or T::Helpers in a shim is unnecessary
end

class MyClass
  extend(T::Helpers)
  ^^^^^^^^^^^^^^^^^^ Sorbet/ForbidExtendTSigHelpersInShims: Extending T::Sig or T::Helpers in a shim is unnecessary
end
