class Foo
  sig { override(allow_incompatible: true).params(a: Integer).void }
                 ^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/AllowIncompatibleOverride: Usage of `allow_incompatible` suggests a violation of the Liskov Substitution Principle. Instead, strive to write interfaces which respect subtyping principles and remove `allow_incompatible`
end
