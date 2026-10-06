sig { params(x: Integer).returns(Integer).on_failure(:raise) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RuntimeOnFailureDependsOnChecked: To use .on_failure you must additionally call .checked(:tests) or .checked(:always), otherwise, the .on_failure has no effect.

sig { params(x: String).returns(String).checked(:none).on_failure(:raise) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RuntimeOnFailureDependsOnChecked: To use .on_failure you must additionally call .checked(:tests) or .checked(:always), otherwise, the .on_failure has no effect.

sig { params(x: String).returns(String).checked().on_failure(:log) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RuntimeOnFailureDependsOnChecked: To use .on_failure you must additionally call .checked(:tests) or .checked(:always), otherwise, the .on_failure has no effect.

sig { params(x: String).returns(String).checked(true).on_failure(:raise) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RuntimeOnFailureDependsOnChecked: To use .on_failure you must additionally call .checked(:tests) or .checked(:always), otherwise, the .on_failure has no effect.

sig { params(x: String).returns(String).checked("tests").on_failure(:log) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RuntimeOnFailureDependsOnChecked: To use .on_failure you must additionally call .checked(:tests) or .checked(:always), otherwise, the .on_failure has no effect.

sig { void.on_failure(:log) }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RuntimeOnFailureDependsOnChecked: To use .on_failure you must additionally call .checked(:tests) or .checked(:always), otherwise, the .on_failure has no effect.
