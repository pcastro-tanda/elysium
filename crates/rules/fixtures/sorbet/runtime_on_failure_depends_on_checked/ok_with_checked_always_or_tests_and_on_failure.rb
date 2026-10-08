sig { params(x: Integer).returns(Integer).checked(:always).on_failure(:raise) }

sig { params(x: Integer).returns(Integer).checked(:tests).on_failure(:log) }

sig { void.checked(:always).on_failure(:raise) }
