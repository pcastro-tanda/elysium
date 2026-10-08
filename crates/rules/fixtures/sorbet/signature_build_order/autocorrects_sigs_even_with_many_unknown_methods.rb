sig { void.foo.type_parameters(:U).bar.params(x: T.type_parameter(:U), y: T::Hash[String, Integer]).baz }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/SignatureBuildOrder: Sig builders must be invoked in the following order: type_parameters, foo, params, bar, void, baz.
