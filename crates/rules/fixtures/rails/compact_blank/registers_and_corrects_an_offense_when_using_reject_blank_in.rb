hash.transform_values { |value| value.reject(&:blank?) }
                                      ^^^^^^^^^^^^^^^^ Use `compact_blank` instead.
