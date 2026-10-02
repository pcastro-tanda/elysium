raise StandardError unless (foo ||= bar) || a = 42
                                              ^^^^ Don't use literal assignment `= 42` in conditional, should be `==` or non-literal operand.
