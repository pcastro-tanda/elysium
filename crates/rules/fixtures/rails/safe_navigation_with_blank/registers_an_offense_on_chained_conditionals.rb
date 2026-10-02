do_something unless foo&.bar&.blank?
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid calling `blank?` with the safe navigation operator in conditionals.
