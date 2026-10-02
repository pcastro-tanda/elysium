do_something unless foo&.blank?
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid calling `blank?` with the safe navigation operator in conditionals.
