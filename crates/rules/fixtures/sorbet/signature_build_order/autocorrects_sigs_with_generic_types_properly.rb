sig { void.type_parameters(:U).params(x: T.type_parameter(:U), y: T::Hash[String, Integer]) }
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/SignatureBuildOrder: Sig builders must be invoked in the following order: type_parameters, params, void.
