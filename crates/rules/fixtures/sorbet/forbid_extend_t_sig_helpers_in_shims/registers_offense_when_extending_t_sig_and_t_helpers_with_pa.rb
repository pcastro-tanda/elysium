module MyModule
  extend(T::Sig)
  ^^^^^^^^^^^^^^ Sorbet/ForbidExtendTSigHelpersInShims: Extending T::Sig or T::Helpers in a shim is unnecessary
  extend(T::Helpers)
  ^^^^^^^^^^^^^^^^^^ Sorbet/ForbidExtendTSigHelpersInShims: Extending T::Sig or T::Helpers in a shim is unnecessary

  sig { returns(String) }
  def foo; end
end
