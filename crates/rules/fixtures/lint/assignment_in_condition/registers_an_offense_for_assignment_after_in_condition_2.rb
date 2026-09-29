raise StandardError unless (foo ||= bar) || a = b
                                              ^ Use `==` if you meant to do a comparison or wrap the expression in parentheses to indicate you meant to assign in a condition.
